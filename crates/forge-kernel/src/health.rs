use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capabilities::HealthState;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbePolicy {
    pub check_id: String,
    pub required: bool,
    pub maximum_age_ms: u64,
    pub degradation_after_failures: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeObservation {
    pub check_id: String,
    pub passed: bool,
    pub observed_at_ms: u64,
    pub latency_ms: u64,
    pub failure_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub state: HealthState,
    pub ready: bool,
    pub failed_required: Vec<String>,
    pub degraded_optional: Vec<String>,
    pub escalated_failures: Vec<String>,
    pub digest: String,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum HealthError {
    #[error("health probe policy is invalid")]
    InvalidPolicy,
    #[error("health probe observation is invalid")]
    InvalidObservation,
    #[error("health probe is not registered")]
    UnknownProbe,
    #[error("health report could not be fingerprinted")]
    Fingerprint,
}

#[derive(Default)]
pub struct HealthRegistry {
    policies: BTreeMap<String, ProbePolicy>,
    latest: BTreeMap<String, ProbeObservation>,
    consecutive_failures: BTreeMap<String, u32>,
}

impl HealthRegistry {
    pub fn register(&mut self, policy: ProbePolicy) -> Result<(), HealthError> {
        if !valid_identity(&policy.check_id)
            || policy.maximum_age_ms == 0
            || policy.degradation_after_failures == 0
            || self.policies.len() >= 128 && !self.policies.contains_key(&policy.check_id)
            || self.policies.contains_key(&policy.check_id)
        {
            return Err(HealthError::InvalidPolicy);
        }
        self.policies.insert(policy.check_id.clone(), policy);
        Ok(())
    }

    pub fn record(&mut self, observation: ProbeObservation) -> Result<(), HealthError> {
        if !self.policies.contains_key(&observation.check_id) {
            return Err(HealthError::UnknownProbe);
        }
        if observation
            .failure_code
            .as_ref()
            .is_some_and(|code| !valid_code(code))
        {
            return Err(HealthError::InvalidObservation);
        }
        if self
            .latest
            .get(&observation.check_id)
            .is_some_and(|previous| observation.observed_at_ms < previous.observed_at_ms)
        {
            return Err(HealthError::InvalidObservation);
        }
        let failures = self
            .consecutive_failures
            .entry(observation.check_id.clone())
            .or_default();
        if observation.passed {
            *failures = 0;
        } else {
            *failures = failures.saturating_add(1);
        }
        self.latest
            .insert(observation.check_id.clone(), observation);
        Ok(())
    }

    pub fn readiness(&self, now_ms: u64) -> Result<ReadinessReport, HealthError> {
        let mut failed_required = Vec::new();
        let mut degraded_optional = Vec::new();
        let mut escalated_failures = Vec::new();
        let mut unknown_required = false;
        for (id, policy) in &self.policies {
            let observation = self.latest.get(id);
            let fresh = observation.is_some_and(|item| {
                item.observed_at_ms <= now_ms
                    && now_ms - item.observed_at_ms <= policy.maximum_age_ms
            });
            let passed = observation.is_some_and(|item| item.passed);
            if policy.required && (!fresh || !passed) {
                if observation.is_none() || !fresh {
                    unknown_required = true;
                }
                failed_required.push(id.clone());
            } else if !policy.required && (!fresh || !passed) {
                degraded_optional.push(id.clone());
            }
            if self.failure_streak(id) >= policy.degradation_after_failures {
                escalated_failures.push(id.clone());
            }
        }
        let state = if !failed_required.is_empty() {
            if unknown_required {
                HealthState::Unknown
            } else {
                HealthState::NotReady
            }
        } else if !degraded_optional.is_empty() {
            HealthState::Degraded
        } else {
            HealthState::Ready
        };
        let report_input = serde_json::json!({
            "state": state,
            "failedRequired": failed_required,
            "degradedOptional": degraded_optional,
            "escalatedFailures": escalated_failures,
            "observations": self.latest,
        });
        let bytes = serde_json::to_vec(&report_input).map_err(|_| HealthError::Fingerprint)?;
        Ok(ReadinessReport {
            state,
            ready: state == HealthState::Ready,
            failed_required,
            degraded_optional,
            escalated_failures,
            digest: blake3::hash(&bytes).to_hex().to_string(),
        })
    }

    pub fn failure_streak(&self, check_id: &str) -> u32 {
        self.consecutive_failures
            .get(check_id)
            .copied()
            .unwrap_or(0)
    }
}

fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        && !value.contains("..")
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_requires_fresh_required_checks_and_degrades_optional_checks() {
        let mut registry = HealthRegistry::default();
        registry
            .register(ProbePolicy {
                check_id: "store.integrity".into(),
                required: true,
                maximum_age_ms: 100,
                degradation_after_failures: 2,
            })
            .expect("required probe");
        registry
            .register(ProbePolicy {
                check_id: "hive.optional".into(),
                required: false,
                maximum_age_ms: 100,
                degradation_after_failures: 2,
            })
            .expect("optional probe");
        registry
            .record(ProbeObservation {
                check_id: "store.integrity".into(),
                passed: true,
                observed_at_ms: 100,
                latency_ms: 2,
                failure_code: None,
            })
            .expect("record");
        let report = registry.readiness(120).expect("report");
        assert_eq!(report.state, HealthState::Degraded);
        assert!(!report.ready);
        assert_eq!(report.degraded_optional, vec!["hive.optional"]);
    }

    #[test]
    fn consecutive_failure_count_resets_after_recovery() {
        let mut registry = HealthRegistry::default();
        registry
            .register(ProbePolicy {
                check_id: "probe".into(),
                required: true,
                maximum_age_ms: 20,
                degradation_after_failures: 2,
            })
            .expect("policy");
        for time in [1, 2] {
            registry
                .record(ProbeObservation {
                    check_id: "probe".into(),
                    passed: false,
                    observed_at_ms: time,
                    latency_ms: 1,
                    failure_code: Some("PROBE_FAILED".into()),
                })
                .expect("failure");
        }
        assert_eq!(registry.failure_streak("probe"), 2);
        assert_eq!(
            registry
                .readiness(3)
                .expect("failure report")
                .escalated_failures,
            vec!["probe"]
        );
        registry
            .record(ProbeObservation {
                check_id: "probe".into(),
                passed: true,
                observed_at_ms: 3,
                latency_ms: 1,
                failure_code: None,
            })
            .expect("recovery");
        assert_eq!(registry.failure_streak("probe"), 0);
        assert_eq!(
            registry.readiness(3).expect("ready report").state,
            HealthState::Ready
        );
    }
}
