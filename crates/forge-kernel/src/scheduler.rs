use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Lane {
    KernelCritical,
    Recovery,
    Interactive,
    Normal,
    Background,
    Telemetry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledWork<T> {
    pub owner: String,
    pub lane: Lane,
    pub value: T,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SchedulerError {
    #[error("scheduler lane configuration is invalid")]
    InvalidConfiguration,
    #[error("scheduler lane is not configured")]
    MissingLane,
    #[error("work queue is at its configured capacity")]
    QueueFull,
    #[error("owner has reached its fair-share queue limit")]
    OwnerLimit,
    #[error("owner identity is empty")]
    InvalidOwner,
}

struct LaneQueue<T> {
    capacity: usize,
    per_owner_capacity: usize,
    queues: BTreeMap<String, VecDeque<ScheduledWork<T>>>,
    owners: VecDeque<String>,
    total: usize,
}

pub struct FairScheduler<T> {
    lanes: BTreeMap<Lane, Arc<Mutex<LaneQueue<T>>>>,
}

impl<T> FairScheduler<T> {
    pub fn new(
        limits: impl IntoIterator<Item = (Lane, usize, usize)>,
    ) -> Result<Self, SchedulerError> {
        let mut lanes = BTreeMap::new();
        for (lane, capacity, per_owner_capacity) in limits {
            if capacity == 0
                || capacity > 100_000
                || per_owner_capacity == 0
                || per_owner_capacity > capacity
                || lanes.contains_key(&lane)
                || lanes.len() >= 6
            {
                return Err(SchedulerError::InvalidConfiguration);
            }
            lanes.insert(
                lane,
                Arc::new(Mutex::new(LaneQueue {
                    capacity,
                    per_owner_capacity,
                    queues: BTreeMap::new(),
                    owners: VecDeque::new(),
                    total: 0,
                })),
            );
        }
        if lanes.is_empty() {
            return Err(SchedulerError::InvalidConfiguration);
        }
        Ok(Self { lanes })
    }

    pub fn enqueue(
        &self,
        owner: impl Into<String>,
        lane: Lane,
        value: T,
    ) -> Result<(), SchedulerError> {
        let owner = owner.into();
        if owner.trim().is_empty()
            || owner.len() > 128
            || !owner
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(SchedulerError::InvalidOwner);
        }
        let queue = self.lanes.get(&lane).ok_or(SchedulerError::MissingLane)?;
        let mut queue = lock_recover(queue);
        if queue.total >= queue.capacity {
            return Err(SchedulerError::QueueFull);
        }
        let owner_len = queue.queues.get(&owner).map(VecDeque::len).unwrap_or(0);
        if owner_len >= queue.per_owner_capacity {
            return Err(SchedulerError::OwnerLimit);
        }
        if owner_len == 0 {
            queue.owners.push_back(owner.clone());
        }
        queue
            .queues
            .entry(owner.clone())
            .or_default()
            .push_back(ScheduledWork { owner, lane, value });
        queue.total += 1;
        Ok(())
    }

    pub fn next(&self, lane: Lane) -> Option<ScheduledWork<T>> {
        let lane_queue = self.lanes.get(&lane)?;
        let mut queue = lock_recover(lane_queue);
        if queue.owners.is_empty() {
            return None;
        }
        let owner = queue.owners.pop_front()?;
        let (work, became_empty) = {
            let owner_queue = queue.queues.get_mut(&owner)?;
            let work = owner_queue.pop_front()?;
            (work, owner_queue.is_empty())
        };
        queue.total = queue.total.saturating_sub(1);
        if became_empty {
            queue.queues.remove(&owner);
        } else {
            queue.owners.push_back(owner);
        }
        Some(work)
    }

    pub fn queue_depth(&self, lane: Lane) -> usize {
        self.lanes
            .get(&lane)
            .map(|queue| lock_recover(queue).total)
            .unwrap_or(0)
    }
}

fn lock_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduler() -> FairScheduler<u8> {
        FairScheduler::new([(Lane::Normal, 4, 2)]).expect("valid lane")
    }

    #[test]
    fn queue_capacity_and_owner_share_are_bounded() {
        let scheduler = scheduler();
        scheduler.enqueue("a", Lane::Normal, 1).expect("first item");
        scheduler
            .enqueue("a", Lane::Normal, 2)
            .expect("second item");
        assert_eq!(
            scheduler.enqueue("a", Lane::Normal, 3),
            Err(SchedulerError::OwnerLimit)
        );
        scheduler
            .enqueue("b", Lane::Normal, 4)
            .expect("other owner");
        scheduler.enqueue("c", Lane::Normal, 5).expect("last slot");
        assert_eq!(
            scheduler.enqueue("d", Lane::Normal, 6),
            Err(SchedulerError::QueueFull)
        );
    }

    #[test]
    fn round_robin_prevents_one_owner_from_monopolizing_a_lane() {
        let scheduler = FairScheduler::new([(Lane::Interactive, 8, 8)]).expect("valid lane");
        scheduler
            .enqueue("busy", Lane::Interactive, 1)
            .expect("enqueue");
        scheduler
            .enqueue("busy", Lane::Interactive, 2)
            .expect("enqueue");
        scheduler
            .enqueue("quiet", Lane::Interactive, 3)
            .expect("enqueue");
        assert_eq!(
            scheduler.next(Lane::Interactive).map(|work| work.value),
            Some(1)
        );
        assert_eq!(
            scheduler.next(Lane::Interactive).map(|work| work.value),
            Some(3)
        );
    }

    #[test]
    fn round_robin_owner_membership_uses_constant_time_rotation() {
        let scheduler = FairScheduler::new([(Lane::Normal, 2_000, 1)]).expect("valid lane");
        for index in 0..1_000 {
            scheduler
                .enqueue(format!("owner.{index}"), Lane::Normal, index)
                .expect("enqueue owner");
        }
        for expected in 0..1_000 {
            assert_eq!(
                scheduler.next(Lane::Normal).map(|work| work.value),
                Some(expected)
            );
        }
        assert_eq!(scheduler.queue_depth(Lane::Normal), 0);
    }

    #[test]
    fn scheduler_rejects_invalid_capacity_configuration() {
        assert!(matches!(
            FairScheduler::<u8>::new([(Lane::Normal, 0, 1)]),
            Err(SchedulerError::InvalidConfiguration)
        ));
        assert!(matches!(
            FairScheduler::<u8>::new([(Lane::Normal, 2, 3)]),
            Err(SchedulerError::InvalidConfiguration)
        ));
        assert!(matches!(
            FairScheduler::<u8>::new([]),
            Err(SchedulerError::InvalidConfiguration)
        ));
    }
}
