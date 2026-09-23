use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use forge_state::{PersistedResourceUsageRecord, ResourceUsagePool};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceVector {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub network_bytes: u64,
    pub tokens: u64,
    pub cost_micros: u64,
}

impl ResourceVector {
    pub fn fits_within(self, limit: Self) -> bool {
        self.cpu_millis <= limit.cpu_millis
            && self.memory_bytes <= limit.memory_bytes
            && self.disk_bytes <= limit.disk_bytes
            && self.network_bytes <= limit.network_bytes
            && self.tokens <= limit.tokens
            && self.cost_micros <= limit.cost_micros
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            cpu_millis: self.cpu_millis.checked_add(other.cpu_millis)?,
            memory_bytes: self.memory_bytes.checked_add(other.memory_bytes)?,
            disk_bytes: self.disk_bytes.checked_add(other.disk_bytes)?,
            network_bytes: self.network_bytes.checked_add(other.network_bytes)?,
            tokens: self.tokens.checked_add(other.tokens)?,
            cost_micros: self.cost_micros.checked_add(other.cost_micros)?,
        })
    }

    fn checked_sub(self, other: Self) -> Option<Self> {
        Some(Self {
            cpu_millis: self.cpu_millis.checked_sub(other.cpu_millis)?,
            memory_bytes: self.memory_bytes.checked_sub(other.memory_bytes)?,
            disk_bytes: self.disk_bytes.checked_sub(other.disk_bytes)?,
            network_bytes: self.network_bytes.checked_sub(other.network_bytes)?,
            tokens: self.tokens.checked_sub(other.tokens)?,
            cost_micros: self.cost_micros.checked_sub(other.cost_micros)?,
        })
    }

    fn budget_capacity_after(self, consumed: Self) -> Option<Self> {
        Some(Self {
            tokens: self.tokens.checked_sub(consumed.tokens)?,
            cost_micros: self.cost_micros.checked_sub(consumed.cost_micros)?,
            ..self
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsageRecord {
    pub owner: String,
    pub category: String,
    pub used: ResourceVector,
    pub cache_tokens_reused: u64,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ResourceError {
    #[error("requested resources exceed an available budget")]
    BudgetExceeded,
    #[error("resource accounting is in progress for this lease")]
    AccountingInProgress,
    #[error("resource accounting requires recovery before new authority can be granted")]
    AccountingReconciliationRequired,
    #[error("resource request is invalid")]
    InvalidRequest,
    #[error("resource lease was revoked")]
    Revoked,
}

#[derive(Clone)]
pub struct ResourceGovernor {
    inner: Arc<Mutex<GovernorState>>,
}

struct GovernorState {
    ordinary_limit: ResourceVector,
    survival_limit: ResourceVector,
    ordinary_reserved: ResourceVector,
    survival_reserved: ResourceVector,
    ordinary_consumed: ResourceVector,
    survival_consumed: ResourceVector,
    usage_records: Vec<ResourceUsageRecord>,
    accounted_usage: BTreeMap<String, PersistedResourceUsageRecord>,
    accounting_dirty: bool,
}

#[derive(Clone)]
pub struct ResourceLease {
    node: Arc<LeaseNode>,
}

struct LeaseNode {
    owner: String,
    grant: ResourceVector,
    remaining: Mutex<ResourceVector>,
    reservation: Arc<RootReservation>,
    accounting_lock: tokio::sync::Mutex<()>,
    revoked: AtomicBool,
    parent: Option<Arc<LeaseNode>>,
}

struct RootReservation {
    governor: Weak<Mutex<GovernorState>>,
    amount: Mutex<ResourceVector>,
    pool: LeasePool,
}

#[derive(Clone, Copy)]
enum LeasePool {
    Ordinary,
    Survival,
}

impl ResourceGovernor {
    pub fn new(limit: ResourceVector) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GovernorState {
                ordinary_limit: limit,
                survival_limit: ResourceVector::default(),
                ordinary_reserved: ResourceVector::default(),
                survival_reserved: ResourceVector::default(),
                ordinary_consumed: ResourceVector::default(),
                survival_consumed: ResourceVector::default(),
                usage_records: Vec::new(),
                accounted_usage: BTreeMap::new(),
                accounting_dirty: false,
            })),
        }
    }

    pub fn with_survival_reserve(
        limit: ResourceVector,
        survival_reserve: ResourceVector,
    ) -> Result<Self, ResourceError> {
        let ordinary_limit = limit
            .checked_sub(survival_reserve)
            .ok_or(ResourceError::InvalidRequest)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(GovernorState {
                ordinary_limit,
                survival_limit: survival_reserve,
                ordinary_reserved: ResourceVector::default(),
                survival_reserved: ResourceVector::default(),
                ordinary_consumed: ResourceVector::default(),
                survival_consumed: ResourceVector::default(),
                usage_records: Vec::new(),
                accounted_usage: BTreeMap::new(),
                accounting_dirty: false,
            })),
        })
    }

    pub(crate) fn from_persisted_usage(
        limit: ResourceVector,
        survival_reserve: ResourceVector,
        records: &[PersistedResourceUsageRecord],
    ) -> Result<Self, ResourceError> {
        if records.len() > 100_000 {
            return Err(ResourceError::InvalidRequest);
        }
        let governor = Self::with_survival_reserve(limit, survival_reserve)?;
        let mut state = lock_recover(&governor.inner);
        for record in records {
            let used_budget = ResourceVector {
                tokens: record.used.tokens,
                cost_micros: record.used.cost_micros,
                ..ResourceVector::default()
            };
            let consumed = match record.pool {
                ResourceUsagePool::Ordinary => &mut state.ordinary_consumed,
                ResourceUsagePool::Survival => &mut state.survival_consumed,
            };
            *consumed = consumed
                .checked_add(used_budget)
                .ok_or(ResourceError::InvalidRequest)?;
            state.usage_records.push(ResourceUsageRecord {
                owner: record.owner.clone(),
                category: record.category.clone(),
                used: ResourceVector {
                    cpu_millis: record.used.cpu_millis,
                    memory_bytes: record.used.memory_bytes,
                    disk_bytes: record.used.disk_bytes,
                    network_bytes: record.used.network_bytes,
                    tokens: record.used.tokens,
                    cost_micros: record.used.cost_micros,
                },
                cache_tokens_reused: record.cache_tokens_reused,
            });
            if state
                .accounted_usage
                .insert(record.usage_id.clone(), record.clone())
                .is_some()
            {
                return Err(ResourceError::InvalidRequest);
            }
        }
        drop(state);
        Ok(governor)
    }

    pub fn try_lease(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
    ) -> Result<ResourceLease, ResourceError> {
        let owner = owner.into();
        if owner.trim().is_empty() || owner.len() > 128 || requested == ResourceVector::default() {
            return Err(ResourceError::InvalidRequest);
        }
        self.try_lease_from_pool(owner, requested, LeasePool::Ordinary)
    }

    pub fn reserved(&self) -> ResourceVector {
        let state = lock_recover(&self.inner);
        state
            .ordinary_reserved
            .checked_add(state.survival_reserved)
            .unwrap_or_else(|| state.ordinary_limit)
    }

    pub fn consumed_budget(&self) -> ResourceVector {
        let state = lock_recover(&self.inner);
        ResourceVector {
            tokens: state
                .ordinary_consumed
                .tokens
                .saturating_add(state.survival_consumed.tokens),
            cost_micros: state
                .ordinary_consumed
                .cost_micros
                .saturating_add(state.survival_consumed.cost_micros),
            ..ResourceVector::default()
        }
    }

    pub fn usage_records(&self) -> Vec<ResourceUsageRecord> {
        lock_recover(&self.inner).usage_records.clone()
    }

    pub(crate) fn accounted_usage_record(
        &self,
        usage_id: &str,
    ) -> Option<PersistedResourceUsageRecord> {
        lock_recover(&self.inner)
            .accounted_usage
            .get(usage_id)
            .cloned()
    }

    pub(crate) fn mark_accounting_dirty(&self) {
        lock_recover(&self.inner).accounting_dirty = true;
    }

    pub(crate) fn accounting_requires_recovery(&self) -> bool {
        lock_recover(&self.inner).accounting_dirty
    }

    pub fn try_survival_lease(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
    ) -> Result<ResourceLease, ResourceError> {
        self.try_lease_from_pool(owner, requested, LeasePool::Survival)
    }

    fn try_lease_from_pool(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
        pool: LeasePool,
    ) -> Result<ResourceLease, ResourceError> {
        let owner = owner.into();
        if !valid_token(&owner) || requested == ResourceVector::default() {
            return Err(ResourceError::InvalidRequest);
        }
        let mut state = lock_recover(&self.inner);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        let (limit, consumed, reserved) = match pool {
            LeasePool::Ordinary => (
                state.ordinary_limit,
                state.ordinary_consumed,
                &mut state.ordinary_reserved,
            ),
            LeasePool::Survival => (
                state.survival_limit,
                state.survival_consumed,
                &mut state.survival_reserved,
            ),
        };
        let capacity = limit
            .budget_capacity_after(consumed)
            .ok_or(ResourceError::BudgetExceeded)?;
        let next = reserved
            .checked_add(requested)
            .ok_or(ResourceError::BudgetExceeded)?;
        if !next.fits_within(capacity) {
            return Err(ResourceError::BudgetExceeded);
        }
        *reserved = next;
        let reservation = Arc::new(RootReservation {
            governor: Arc::downgrade(&self.inner),
            amount: Mutex::new(requested),
            pool,
        });
        Ok(ResourceLease {
            node: Arc::new(LeaseNode {
                owner,
                grant: requested,
                remaining: Mutex::new(requested),
                reservation,
                accounting_lock: tokio::sync::Mutex::new(()),
                revoked: AtomicBool::new(false),
                parent: None,
            }),
        })
    }
}

impl ResourceLease {
    pub fn owner(&self) -> &str {
        &self.node.owner
    }

    pub fn grant(&self) -> ResourceVector {
        self.node.grant
    }

    pub fn remaining(&self) -> ResourceVector {
        *lock_recover(&self.node.remaining)
    }

    pub fn ensure_active(&self) -> Result<(), ResourceError> {
        let _accounting_guard = self
            .node
            .accounting_lock
            .try_lock()
            .map_err(|_| ResourceError::AccountingInProgress)?;
        let governor = self
            .node
            .reservation
            .governor
            .upgrade()
            .ok_or(ResourceError::InvalidRequest)?;
        let state = lock_recover(&governor);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        self.ensure_not_revoked()
    }

    pub fn revoke(&self) {
        if let Some(governor) = self.node.reservation.governor.upgrade() {
            let _state = lock_recover(&governor);
            self.node.revoked.store(true, Ordering::Release);
            return;
        }
        self.node.revoked.store(true, Ordering::Release);
    }

    pub(crate) async fn lock_for_accounting(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.node.accounting_lock.lock().await
    }

    pub(crate) fn validate_charge(
        &self,
        category: &str,
        used: ResourceVector,
        cache_tokens_reused: u64,
    ) -> Result<(), ResourceError> {
        if !valid_token(category) || used == ResourceVector::default() && cache_tokens_reused == 0 {
            return Err(ResourceError::InvalidRequest);
        }
        let governor = self
            .node
            .reservation
            .governor
            .upgrade()
            .ok_or(ResourceError::InvalidRequest)?;
        let state = lock_recover(&governor);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        self.ensure_not_revoked()?;
        if state.usage_records.len() >= 100_000 {
            return Err(ResourceError::InvalidRequest);
        }
        lock_recover(&self.node.remaining)
            .checked_sub(used)
            .ok_or(ResourceError::BudgetExceeded)?;
        let used_budget = ResourceVector {
            tokens: used.tokens,
            cost_micros: used.cost_micros,
            ..ResourceVector::default()
        };
        let reservation_amount = lock_recover(&self.node.reservation.amount);
        reservation_amount
            .checked_sub(used_budget)
            .ok_or(ResourceError::BudgetExceeded)?;
        let (limit, consumed) = match self.node.reservation.pool {
            LeasePool::Ordinary => (state.ordinary_limit, state.ordinary_consumed),
            LeasePool::Survival => (state.survival_limit, state.survival_consumed),
        };
        let next_consumed = consumed
            .checked_add(used_budget)
            .ok_or(ResourceError::BudgetExceeded)?;
        if next_consumed.tokens > limit.tokens || next_consumed.cost_micros > limit.cost_micros {
            return Err(ResourceError::BudgetExceeded);
        }
        Ok(())
    }

    pub(crate) fn pool(&self) -> ResourceUsagePool {
        match self.node.reservation.pool {
            LeasePool::Ordinary => ResourceUsagePool::Ordinary,
            LeasePool::Survival => ResourceUsagePool::Survival,
        }
    }

    pub(crate) fn pool_limit(&self) -> Result<ResourceVector, ResourceError> {
        let governor = self
            .node
            .reservation
            .governor
            .upgrade()
            .ok_or(ResourceError::InvalidRequest)?;
        let state = lock_recover(&governor);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        Ok(match self.node.reservation.pool {
            LeasePool::Ordinary => state.ordinary_limit,
            LeasePool::Survival => state.survival_limit,
        })
    }

    #[cfg(test)]
    pub(crate) fn charge(
        &self,
        category: &str,
        used: ResourceVector,
        cache_tokens_reused: u64,
    ) -> Result<(), ResourceError> {
        self.charge_inner(category, used, cache_tokens_reused, None)
    }

    pub(crate) fn charge_persisted(
        &self,
        record: &PersistedResourceUsageRecord,
    ) -> Result<(), ResourceError> {
        let pool = match self.node.reservation.pool {
            LeasePool::Ordinary => ResourceUsagePool::Ordinary,
            LeasePool::Survival => ResourceUsagePool::Survival,
        };
        if record.owner != self.node.owner || record.pool != pool {
            return Err(ResourceError::InvalidRequest);
        }
        let used = ResourceVector {
            cpu_millis: record.used.cpu_millis,
            memory_bytes: record.used.memory_bytes,
            disk_bytes: record.used.disk_bytes,
            network_bytes: record.used.network_bytes,
            tokens: record.used.tokens,
            cost_micros: record.used.cost_micros,
        };
        self.charge_inner(
            &record.category,
            used,
            record.cache_tokens_reused,
            Some(record),
        )
    }

    fn charge_inner(
        &self,
        category: &str,
        used: ResourceVector,
        cache_tokens_reused: u64,
        persisted_record: Option<&PersistedResourceUsageRecord>,
    ) -> Result<(), ResourceError> {
        if !valid_token(category) || used == ResourceVector::default() && cache_tokens_reused == 0 {
            return Err(ResourceError::InvalidRequest);
        }
        let governor = self
            .node
            .reservation
            .governor
            .upgrade()
            .ok_or(ResourceError::InvalidRequest)?;
        let mut state = lock_recover(&governor);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        self.ensure_not_revoked()?;
        if state.usage_records.len() >= 100_000 {
            return Err(ResourceError::InvalidRequest);
        }
        if persisted_record
            .is_some_and(|record| state.accounted_usage.contains_key(&record.usage_id))
        {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        let mut remaining = lock_recover(&self.node.remaining);
        let next_remaining = remaining
            .checked_sub(used)
            .ok_or(ResourceError::BudgetExceeded)?;
        let used_budget = ResourceVector {
            tokens: used.tokens,
            cost_micros: used.cost_micros,
            ..ResourceVector::default()
        };
        let mut reservation_amount = lock_recover(&self.node.reservation.amount);
        let next_reservation_amount = reservation_amount
            .checked_sub(used_budget)
            .ok_or(ResourceError::BudgetExceeded)?;
        let (limit, consumed, reserved) = match self.node.reservation.pool {
            LeasePool::Ordinary => (
                state.ordinary_limit,
                state.ordinary_consumed,
                state.ordinary_reserved,
            ),
            LeasePool::Survival => (
                state.survival_limit,
                state.survival_consumed,
                state.survival_reserved,
            ),
        };
        let next_consumed = consumed
            .checked_add(used_budget)
            .ok_or(ResourceError::BudgetExceeded)?;
        if next_consumed.tokens > limit.tokens || next_consumed.cost_micros > limit.cost_micros {
            return Err(ResourceError::BudgetExceeded);
        }
        let next_reserved = reserved
            .checked_sub(used_budget)
            .ok_or(ResourceError::BudgetExceeded)?;
        match self.node.reservation.pool {
            LeasePool::Ordinary => {
                state.ordinary_consumed = next_consumed;
                state.ordinary_reserved = next_reserved;
            }
            LeasePool::Survival => {
                state.survival_consumed = next_consumed;
                state.survival_reserved = next_reserved;
            }
        }
        *reservation_amount = next_reservation_amount;
        *remaining = next_remaining;
        state.usage_records.push(ResourceUsageRecord {
            owner: self.node.owner.clone(),
            category: category.to_owned(),
            used,
            cache_tokens_reused,
        });
        if let Some(record) = persisted_record {
            state
                .accounted_usage
                .insert(record.usage_id.clone(), record.clone());
        }
        Ok(())
    }

    pub fn delegate(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
    ) -> Result<ResourceLease, ResourceError> {
        let _accounting_guard = self
            .node
            .accounting_lock
            .try_lock()
            .map_err(|_| ResourceError::AccountingInProgress)?;
        let owner = owner.into();
        if !valid_token(&owner) || requested == ResourceVector::default() {
            return Err(ResourceError::InvalidRequest);
        }
        let governor = self
            .node
            .reservation
            .governor
            .upgrade()
            .ok_or(ResourceError::InvalidRequest)?;
        let state = lock_recover(&governor);
        if state.accounting_dirty {
            return Err(ResourceError::AccountingReconciliationRequired);
        }
        self.ensure_not_revoked()?;
        let mut remaining = lock_recover(&self.node.remaining);
        let next = remaining
            .checked_sub(requested)
            .ok_or(ResourceError::BudgetExceeded)?;
        *remaining = next;
        Ok(ResourceLease {
            node: Arc::new(LeaseNode {
                owner,
                grant: requested,
                remaining: Mutex::new(requested),
                reservation: Arc::clone(&self.node.reservation),
                accounting_lock: tokio::sync::Mutex::new(()),
                revoked: AtomicBool::new(false),
                parent: Some(Arc::clone(&self.node)),
            }),
        })
    }

    fn ensure_not_revoked(&self) -> Result<(), ResourceError> {
        if self.node.is_revoked() {
            Err(ResourceError::Revoked)
        } else {
            Ok(())
        }
    }
}

impl LeaseNode {
    fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.is_revoked())
    }
}

impl Drop for LeaseNode {
    fn drop(&mut self) {
        if let Some(parent) = self.parent.as_ref() {
            let mut remaining = lock_recover(&parent.remaining);
            if let Some(next) = remaining.checked_add(*lock_recover(&self.remaining)) {
                *remaining = next;
            }
        }
    }
}

impl Drop for RootReservation {
    fn drop(&mut self) {
        if let Some(governor) = self.governor.upgrade() {
            let mut state = lock_recover(&governor);
            let reserved = match self.pool {
                LeasePool::Ordinary => &mut state.ordinary_reserved,
                LeasePool::Survival => &mut state.survival_reserved,
            };
            if let Some(next) = reserved.checked_sub(*lock_recover(&self.amount)) {
                *reserved = next;
            }
        }
    }
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn lock_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::sync::Barrier;

    #[test]
    fn child_budget_cannot_exceed_parent_and_returns_when_released() {
        let governor = ResourceGovernor::new(ResourceVector {
            cpu_millis: 100,
            memory_bytes: 1024,
            ..Default::default()
        });
        let parent = governor
            .try_lease(
                "work-order",
                ResourceVector {
                    cpu_millis: 80,
                    memory_bytes: 800,
                    ..Default::default()
                },
            )
            .expect("parent lease");
        assert_eq!(
            parent
                .delegate(
                    "child",
                    ResourceVector {
                        cpu_millis: 81,
                        memory_bytes: 1,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        let child = parent
            .delegate(
                "child",
                ResourceVector {
                    cpu_millis: 30,
                    memory_bytes: 200,
                    ..Default::default()
                },
            )
            .expect("bounded child");
        assert_eq!(parent.remaining().cpu_millis, 50);
        drop(child);
        assert_eq!(parent.remaining().cpu_millis, 80);
        assert_eq!(governor.reserved().cpu_millis, 80);
        drop(parent);
        assert_eq!(governor.reserved().cpu_millis, 0);
    }

    #[test]
    fn concurrent_resource_reservations_never_over_admit_the_root_limit() {
        const WORKERS: usize = 128;
        const LIMIT: u64 = 64;
        let governor = ResourceGovernor::new(ResourceVector {
            tokens: LIMIT,
            ..Default::default()
        });
        let barrier = Arc::new(Barrier::new(WORKERS + 1));
        let workers = (0..WORKERS)
            .map(|index| {
                let governor = governor.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    governor.try_lease(
                        format!("worker.{index}"),
                        ResourceVector {
                            tokens: 1,
                            ..Default::default()
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let leases = workers
            .into_iter()
            .map(|worker| worker.join().expect("reservation worker exits"))
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        assert_eq!(leases.len(), LIMIT as usize);
        assert_eq!(governor.reserved().tokens, LIMIT);
        drop(leases);
        assert_eq!(governor.reserved().tokens, 0);
    }

    #[test]
    fn revoked_parent_prevents_new_delegation() {
        let governor = ResourceGovernor::new(ResourceVector {
            tokens: 10,
            ..Default::default()
        });
        let parent = governor
            .try_lease(
                "project",
                ResourceVector {
                    tokens: 10,
                    ..Default::default()
                },
            )
            .expect("root lease");
        parent.revoke();
        assert_eq!(parent.ensure_active(), Err(ResourceError::Revoked));
        assert_eq!(
            parent
                .delegate(
                    "child",
                    ResourceVector {
                        tokens: 1,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::Revoked)
        );
    }

    #[test]
    fn child_revocation_does_not_revoke_its_parent_or_sibling_lease() {
        let governor = ResourceGovernor::new(ResourceVector {
            tokens: 30,
            ..Default::default()
        });
        let parent = governor
            .try_lease(
                "project",
                ResourceVector {
                    tokens: 30,
                    ..Default::default()
                },
            )
            .expect("root lease");
        let child = parent
            .delegate(
                "child-a",
                ResourceVector {
                    tokens: 10,
                    ..Default::default()
                },
            )
            .expect("child a");
        let sibling = parent
            .delegate(
                "child-b",
                ResourceVector {
                    tokens: 10,
                    ..Default::default()
                },
            )
            .expect("child b");
        child.revoke();
        assert_eq!(child.ensure_active(), Err(ResourceError::Revoked));
        assert!(parent.ensure_active().is_ok());
        assert!(sibling.ensure_active().is_ok());
    }

    #[test]
    fn nested_lease_keeps_its_budget_reserved_until_descendants_release() {
        let governor = ResourceGovernor::new(ResourceVector {
            tokens: 100,
            ..Default::default()
        });
        let root = governor
            .try_lease(
                "root",
                ResourceVector {
                    tokens: 100,
                    ..Default::default()
                },
            )
            .expect("root");
        let child = root
            .delegate(
                "child",
                ResourceVector {
                    tokens: 80,
                    ..Default::default()
                },
            )
            .expect("child");
        let grandchild = child
            .delegate(
                "grandchild",
                ResourceVector {
                    tokens: 70,
                    ..Default::default()
                },
            )
            .expect("grandchild");

        drop(child);
        assert_eq!(root.remaining().tokens, 20);
        assert_eq!(
            root.delegate(
                "overlap",
                ResourceVector {
                    tokens: 21,
                    ..Default::default()
                }
            )
            .err(),
            Some(ResourceError::BudgetExceeded)
        );

        drop(grandchild);
        assert_eq!(root.remaining().tokens, 100);
    }

    #[test]
    fn ordinary_work_cannot_borrow_survival_reserve() {
        let governor = ResourceGovernor::with_survival_reserve(
            ResourceVector {
                tokens: 100,
                cost_micros: 1_000,
                ..Default::default()
            },
            ResourceVector {
                tokens: 20,
                cost_micros: 200,
                ..Default::default()
            },
        )
        .expect("valid survival reserve");

        assert_eq!(
            governor
                .try_lease(
                    "ordinary.too_large",
                    ResourceVector {
                        tokens: 81,
                        cost_micros: 801,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        let ordinary = governor
            .try_lease(
                "ordinary",
                ResourceVector {
                    tokens: 80,
                    cost_micros: 800,
                    ..Default::default()
                },
            )
            .expect("ordinary pool remains available");
        assert_eq!(
            governor
                .try_lease(
                    "ordinary.borrow",
                    ResourceVector {
                        tokens: 1,
                        cost_micros: 1,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        let survival = governor
            .try_survival_lease(
                "survival",
                ResourceVector {
                    tokens: 20,
                    cost_micros: 200,
                    ..Default::default()
                },
            )
            .expect("protected survival pool remains available");
        assert_eq!(governor.reserved().tokens, 100);
        drop(survival);
        drop(ordinary);
    }

    #[test]
    fn survival_leases_are_limited_to_the_protected_pool() {
        let governor = ResourceGovernor::with_survival_reserve(
            ResourceVector {
                tokens: 100,
                cost_micros: 1_000,
                ..Default::default()
            },
            ResourceVector {
                tokens: 20,
                cost_micros: 200,
                ..Default::default()
            },
        )
        .expect("valid survival reserve");

        assert_eq!(
            governor
                .try_survival_lease(
                    "survival.too_large",
                    ResourceVector {
                        tokens: 21,
                        cost_micros: 201,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        let survival = governor
            .try_survival_lease(
                "survival",
                ResourceVector {
                    tokens: 20,
                    cost_micros: 200,
                    ..Default::default()
                },
            )
            .expect("reserve lease");
        assert_eq!(
            governor
                .try_survival_lease(
                    "survival.exhausted",
                    ResourceVector {
                        tokens: 1,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        assert!(
            governor
                .try_lease(
                    "ordinary",
                    ResourceVector {
                        tokens: 80,
                        cost_micros: 800,
                        ..Default::default()
                    }
                )
                .is_ok()
        );
        drop(survival);
    }

    #[test]
    fn charges_are_attributed_and_budget_consumption_is_cumulative() {
        let governor = ResourceGovernor::new(ResourceVector {
            tokens: 100,
            cost_micros: 1_000,
            ..Default::default()
        });
        let root = governor
            .try_lease(
                "work_order",
                ResourceVector {
                    tokens: 100,
                    cost_micros: 1_000,
                    ..Default::default()
                },
            )
            .expect("root lease");
        let child = root
            .delegate(
                "executor",
                ResourceVector {
                    tokens: 60,
                    cost_micros: 600,
                    ..Default::default()
                },
            )
            .expect("child lease");
        child
            .charge(
                "llm.cache",
                ResourceVector {
                    tokens: 25,
                    cost_micros: 200,
                    ..Default::default()
                },
                100,
            )
            .expect("record usage");

        assert_eq!(
            governor.usage_records(),
            vec![ResourceUsageRecord {
                owner: "executor".into(),
                category: "llm.cache".into(),
                used: ResourceVector {
                    tokens: 25,
                    cost_micros: 200,
                    ..Default::default()
                },
                cache_tokens_reused: 100,
            }]
        );
        assert_eq!(
            governor.consumed_budget(),
            ResourceVector {
                tokens: 25,
                cost_micros: 200,
                ..Default::default()
            }
        );
        drop(child);
        drop(root);

        assert_eq!(governor.reserved(), ResourceVector::default());
        assert_eq!(
            governor
                .try_lease(
                    "over_budget",
                    ResourceVector {
                        tokens: 76,
                        ..Default::default()
                    }
                )
                .err(),
            Some(ResourceError::BudgetExceeded)
        );
        assert!(
            governor
                .try_lease(
                    "remaining_budget",
                    ResourceVector {
                        tokens: 75,
                        cost_micros: 800,
                        ..Default::default()
                    }
                )
                .is_ok()
        );
    }

    proptest! {
        #[test]
        fn reservations_never_exceed_the_root_ceiling(limit in 0_u64..10_000, request in 0_u64..20_000) {
            let governor = ResourceGovernor::new(ResourceVector { tokens: limit, ..Default::default() });
            let result = governor.try_lease("property", ResourceVector { tokens: request, ..Default::default() });
            prop_assert_eq!(result.is_ok(), request != 0 && request <= limit);
            prop_assert!(governor.reserved().tokens <= limit);
        }
    }
}
