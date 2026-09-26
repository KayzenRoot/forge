use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use forge_state::{ForgeStateStore, StateError, StoredEvent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::broadcast;

use crate::capabilities::PrivacyClass;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    EphemeralLocal,
    DurableLocal,
    Integration,
    AuditEvidence,
    Telemetry,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLane {
    CriticalLifecycle,
    DurableDomain,
    Integration,
    Telemetry,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub class: EventClass,
    pub lane: EventLane,
    pub producer: String,
    pub privacy: PrivacyClass,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub execution_id: Option<String>,
    pub occurred_at_ms: u64,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishReceipt {
    pub durable: bool,
    pub receiver_count: usize,
    pub event_id: String,
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("event envelope is invalid")]
    InvalidEnvelope,
    #[error("event lane is not configured")]
    MissingLane,
    #[error("ephemeral and durable event APIs were mixed")]
    ClassMismatch,
    #[error("event persistence failed")]
    State(#[from] StateError),
    #[error("event sequence is exhausted")]
    SequenceExhausted,
    #[error("event payload exceeds the local privacy boundary")]
    PrivacyDenied,
}

pub struct EventBus {
    lanes: BTreeMap<EventLane, broadcast::Sender<EventEnvelope>>,
    next_sequence: AtomicU64,
}

impl EventBus {
    pub fn new(
        capacities: impl IntoIterator<Item = (EventLane, usize)>,
    ) -> Result<Self, EventError> {
        let mut lanes = BTreeMap::new();
        for (lane, capacity) in capacities {
            if capacity == 0 || capacity > 100_000 || lanes.contains_key(&lane) {
                return Err(EventError::InvalidEnvelope);
            }
            let (sender, _) = broadcast::channel(capacity);
            lanes.insert(lane, sender);
        }
        if lanes.is_empty() {
            return Err(EventError::MissingLane);
        }
        Ok(Self {
            lanes,
            next_sequence: AtomicU64::new(1),
        })
    }

    pub fn subscribe(
        &self,
        lane: EventLane,
    ) -> Result<broadcast::Receiver<EventEnvelope>, EventError> {
        self.lanes
            .get(&lane)
            .map(broadcast::Sender::subscribe)
            .ok_or(EventError::MissingLane)
    }

    pub fn publish_ephemeral(
        &self,
        mut event: EventEnvelope,
    ) -> Result<PublishReceipt, EventError> {
        if event.class != EventClass::EphemeralLocal && event.class != EventClass::Telemetry {
            return Err(EventError::ClassMismatch);
        }
        self.validate(&event)?;
        let sender = self.lanes.get(&event.lane).ok_or(EventError::MissingLane)?;
        self.stamp(&mut event)?;
        let event_id = event.event_id.clone();
        let receiver_count = sender.send(event).unwrap_or(0);
        Ok(PublishReceipt {
            durable: false,
            receiver_count,
            event_id,
        })
    }

    pub async fn publish_durable(
        &self,
        store: &ForgeStateStore,
        event: EventEnvelope,
    ) -> Result<PublishReceipt, EventError> {
        if !matches!(
            event.class,
            EventClass::DurableLocal | EventClass::Integration | EventClass::AuditEvidence
        ) {
            return Err(EventError::ClassMismatch);
        }
        self.validate(&event)?;
        let sender = self.lanes.get(&event.lane).ok_or(EventError::MissingLane)?;
        let event_id = event.event_id.clone();
        store
            .enqueue_event(&StoredEvent {
                event_id: event.event_id.clone(),
                contract_id: event.contract_id.clone(),
                payload: serde_json::to_value(&event).map_err(|_| EventError::InvalidEnvelope)?,
            })
            .await?;
        let receiver_count = sender.send(event).unwrap_or(0);
        Ok(PublishReceipt {
            durable: true,
            receiver_count,
            event_id,
        })
    }

    pub fn validate_durable_before_commit(&self, event: &EventEnvelope) -> Result<(), EventError> {
        if !matches!(
            event.class,
            EventClass::DurableLocal | EventClass::Integration | EventClass::AuditEvidence
        ) {
            return Err(EventError::ClassMismatch);
        }
        self.validate(event)?;
        if !self.lanes.contains_key(&event.lane) {
            return Err(EventError::MissingLane);
        }
        Ok(())
    }

    pub fn validate_ephemeral_before_commit(
        &self,
        event: &EventEnvelope,
    ) -> Result<(), EventError> {
        if event.class != EventClass::EphemeralLocal && event.class != EventClass::Telemetry {
            return Err(EventError::ClassMismatch);
        }
        self.validate(event)?;
        if !self.lanes.contains_key(&event.lane) {
            return Err(EventError::MissingLane);
        }
        Ok(())
    }

    /// Publishes an event whose durable outbox row was committed with its command receipt.
    pub fn publish_committed(&self, event: EventEnvelope) -> Result<PublishReceipt, EventError> {
        self.validate_durable_before_commit(&event)?;
        let sender = self.lanes.get(&event.lane).ok_or(EventError::MissingLane)?;
        let event_id = event.event_id.clone();
        let receiver_count = sender.send(event).unwrap_or(0);
        Ok(PublishReceipt {
            durable: true,
            receiver_count,
            event_id,
        })
    }

    pub async fn replay_pending(
        &self,
        store: &ForgeStateStore,
        limit: u32,
    ) -> Result<Vec<EventEnvelope>, EventError> {
        store
            .pending_events(limit)
            .await?
            .into_iter()
            .map(|stored| {
                serde_json::from_value(stored.payload).map_err(|_| EventError::InvalidEnvelope)
            })
            .collect()
    }

    pub async fn acknowledge(
        &self,
        store: &ForgeStateStore,
        event_id: &str,
    ) -> Result<bool, EventError> {
        store
            .mark_event_delivered(event_id)
            .await
            .map_err(EventError::State)
    }

    fn stamp(&self, event: &mut EventEnvelope) -> Result<(), EventError> {
        let sequence = self
            .next_sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| EventError::SequenceExhausted)?;
        event.event_id = format!("{}-{sequence:016x}", event.event_id);
        Ok(())
    }

    fn validate(&self, event: &EventEnvelope) -> Result<(), EventError> {
        forge_state::validate_payload_for_persistence(&event.payload)
            .map_err(|_| EventError::PrivacyDenied)?;
        let valid = !event.event_id.trim().is_empty()
            && event.event_id.len() <= 160
            && !event.contract_id.trim().is_empty()
            && event.contract_id.len() <= 160
            && !event.contract_version.trim().is_empty()
            && !event.producer.trim().is_empty()
            && event.privacy <= PrivacyClass::Internal
            && serde_json::to_vec(&event.payload).is_ok_and(|payload| payload.len() <= 1_048_576);
        if event.privacy > PrivacyClass::Internal {
            Err(EventError::PrivacyDenied)
        } else if valid {
            Ok(())
        } else {
            Err(EventError::InvalidEnvelope)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(class: EventClass, lane: EventLane) -> EventEnvelope {
        EventEnvelope {
            event_id: "event".into(),
            contract_id: "forge.event.boot".into(),
            contract_version: "1.0.0".into(),
            class,
            lane,
            producer: "forge.kernel".into(),
            privacy: PrivacyClass::Internal,
            causation_id: None,
            correlation_id: None,
            execution_id: Some("execution-1".into()),
            occurred_at_ms: 10,
            payload: json!({"state": "ready"}),
        }
    }

    #[test]
    fn ephemeral_lanes_are_bounded_and_lag_is_visible_to_consumers() {
        let bus = EventBus::new([(EventLane::Telemetry, 2)]).expect("event bus");
        let mut receiver = bus.subscribe(EventLane::Telemetry).expect("subscriber");
        for _ in 0..3 {
            bus.publish_ephemeral(event(EventClass::Telemetry, EventLane::Telemetry))
                .expect("publish");
        }
        assert!(matches!(
            receiver.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(1))
        ));
        assert_eq!(
            receiver.try_recv().expect("retained event").class,
            EventClass::Telemetry
        );
    }

    #[tokio::test]
    async fn durable_events_are_persisted_before_fanout_and_replayable() {
        let bus = EventBus::new([(EventLane::DurableDomain, 2)]).expect("bus");
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("state store");
        let event = event(EventClass::DurableLocal, EventLane::DurableDomain);
        let receipt = bus
            .publish_durable(&store, event)
            .await
            .expect("durable publish");
        assert!(receipt.durable);
        let pending = bus.replay_pending(&store, 10).await.expect("replay");
        assert_eq!(pending.len(), 1);
        assert!(
            bus.acknowledge(&store, &pending[0].event_id)
                .await
                .expect("acknowledge")
        );
        assert!(
            bus.replay_pending(&store, 10)
                .await
                .expect("empty replay")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn missing_lane_does_not_persist_a_durable_event() {
        let bus = EventBus::new([(EventLane::Telemetry, 2)]).expect("bus");
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("state store");

        assert!(matches!(
            bus.publish_durable(
                &store,
                event(EventClass::DurableLocal, EventLane::DurableDomain)
            )
            .await,
            Err(EventError::MissingLane)
        ));
        assert!(
            bus.replay_pending(&store, 10)
                .await
                .expect("replay")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn durable_event_ids_are_stable_and_sensitive_payloads_are_rejected() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let event = event(EventClass::DurableLocal, EventLane::DurableDomain);
        let first_bus = EventBus::new([(EventLane::DurableDomain, 4)]).expect("first bus");
        let first = first_bus
            .publish_durable(&store, event.clone())
            .await
            .expect("first durable event");
        let restarted_bus = EventBus::new([(EventLane::DurableDomain, 4)]).expect("restarted bus");
        let replay = restarted_bus
            .publish_durable(&store, event.clone())
            .await
            .expect("idempotent event replay");
        assert_eq!(first.event_id, replay.event_id);
        assert_eq!(store.pending_events(10).await.expect("outbox").len(), 1);

        let mut secret_event = event;
        secret_event.event_id = "private-event".into();
        secret_event.privacy = PrivacyClass::Secret;
        assert!(matches!(
            restarted_bus.publish_durable(&store, secret_event).await,
            Err(EventError::PrivacyDenied)
        ));
        assert_eq!(
            store
                .pending_events(10)
                .await
                .expect("unchanged outbox")
                .len(),
            1
        );
    }

    #[test]
    fn durable_class_cannot_bypass_the_outbox_api() {
        let bus = EventBus::new([(EventLane::DurableDomain, 2)]).expect("bus");
        assert!(matches!(
            bus.publish_ephemeral(event(EventClass::AuditEvidence, EventLane::DurableDomain)),
            Err(EventError::ClassMismatch)
        ));
    }
}
