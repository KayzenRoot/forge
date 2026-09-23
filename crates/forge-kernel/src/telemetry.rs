use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const HISTOGRAM_UPPER_BOUNDS_US: [u64; 6] = [100, 1_000, 10_000, 100_000, 1_000_000, u64::MAX];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub name: String,
    pub counter: u64,
    pub observations: u64,
    pub sum_us: u64,
    pub histogram_buckets: Vec<u64>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TelemetryError {
    #[error("metric name is invalid")]
    InvalidName,
    #[error("hot-path metric registry is at capacity")]
    Capacity,
}

#[derive(Default)]
struct Metric {
    counter: u64,
    observations: u64,
    sum_us: u64,
    histogram_buckets: [u64; 6],
}

pub struct HotPathRegistry {
    capacity: usize,
    metrics: Mutex<BTreeMap<String, Metric>>,
}

impl HotPathRegistry {
    pub fn new(capacity: usize) -> Result<Self, TelemetryError> {
        if capacity == 0 || capacity > 10_000 {
            return Err(TelemetryError::Capacity);
        }
        Ok(Self {
            capacity,
            metrics: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn increment(&self, name: &str, amount: u64) -> Result<(), TelemetryError> {
        let mut metrics = lock_recover(&self.metrics);
        let metric = self.metric(&mut metrics, name)?;
        metric.counter = metric.counter.saturating_add(amount);
        Ok(())
    }

    pub fn observe_micros(&self, name: &str, duration_us: u64) -> Result<(), TelemetryError> {
        let mut metrics = lock_recover(&self.metrics);
        let metric = self.metric(&mut metrics, name)?;
        metric.observations = metric.observations.saturating_add(1);
        metric.sum_us = metric.sum_us.saturating_add(duration_us);
        let bucket = HISTOGRAM_UPPER_BOUNDS_US
            .iter()
            .position(|bound| duration_us <= *bound)
            .unwrap_or(5);
        metric.histogram_buckets[bucket] = metric.histogram_buckets[bucket].saturating_add(1);
        Ok(())
    }

    pub fn snapshot(&self) -> Vec<MetricSnapshot> {
        lock_recover(&self.metrics)
            .iter()
            .map(|(name, metric)| MetricSnapshot {
                name: name.clone(),
                counter: metric.counter,
                observations: metric.observations,
                sum_us: metric.sum_us,
                histogram_buckets: metric.histogram_buckets.to_vec(),
            })
            .collect()
    }

    fn metric<'a>(
        &self,
        metrics: &'a mut BTreeMap<String, Metric>,
        name: &str,
    ) -> Result<&'a mut Metric, TelemetryError> {
        if !valid_metric_name(name) {
            return Err(TelemetryError::InvalidName);
        }
        if metrics.len() >= self.capacity && !metrics.contains_key(name) {
            return Err(TelemetryError::Capacity);
        }
        Ok(metrics.entry(name.to_owned()).or_default())
    }
}

fn valid_metric_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 96
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_bounds_cardinality_and_aggregates_latency_without_labels() {
        let registry = HotPathRegistry::new(2).expect("registry");
        registry.increment("kernel.boot", 2).expect("counter");
        registry
            .observe_micros("kernel.dispatch", 250)
            .expect("observation");
        assert_eq!(
            registry.increment("caller.secret@email", 1),
            Err(TelemetryError::InvalidName)
        );
        assert_eq!(
            registry.increment("cache.hit", 1),
            Err(TelemetryError::Capacity)
        );
        let metrics = registry.snapshot();
        assert_eq!(metrics[1].observations, 1);
        assert_eq!(metrics[1].histogram_buckets[1], 1);
    }
}
