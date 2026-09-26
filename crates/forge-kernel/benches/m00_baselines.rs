use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use forge_contracts::{
    ContractDefinition, ContractId, ContractVersion, Fingerprint, FingerprintKind,
    ValidatedContract,
};
use forge_kernel::cache::{
    CacheAccessCounters, CacheIdentity, get_observed as cache_get_observed, put as cache_put,
};
use forge_kernel::capabilities::{
    CapabilityDescriptor, CapabilityRegistry, EvidenceLevel, HealthState, Locality, PrivacyClass,
    SideEffectClass,
};
use forge_kernel::causality::{
    CausalityGraph, ChangeKind, EdgeConfidence, EdgeKind, GraphEdge, GraphNode, NodeKind,
};
use forge_kernel::commands::{
    CommandBus, CommandContext, CommandRegistration, CommandRequest, HandlerOutput,
    HostAuthorizationClaims, HostDecisionSigner, HostDecisionVerifier, IdempotencyMode,
    SensitivityClass,
};
use forge_kernel::events::{EventBus, EventClass, EventEnvelope, EventLane};
use forge_kernel::health::{HealthRegistry, ProbeObservation, ProbePolicy};
use forge_kernel::proof::{
    ChangeAssessment, ProofBackendClaims, ProofBackendSigner, ProofBackendVerifier, ProofGraph,
    ProofKind, ProofNode, ProofObligationCompiler, ProofOutcome,
};
use forge_kernel::resolver::{ResolutionRequest, resolve};
use forge_kernel::resources::{ResourceGovernor, ResourceVector};
use forge_kernel::runtime::{CancellationToken, Deadline, KernelRuntime, RuntimeConfig};
use forge_kernel::scheduler::{FairScheduler, Lane};
use forge_kernel::telemetry::HotPathRegistry;
use forge_state::{
    ForgeStateStore, PersistedResourceUsageRecord, PersistedResourceVector,
    ResourceUsageAttribution, ResourceUsagePool,
};
use serde_json::{Value, json};
use tokio::runtime::Builder;

fn main() {
    println!(
        "profile os={} arch={} workload=m00-microbench release=true toolchain=see-rustc-vv",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    measure("blake3_1k", 100_000, || {
        black_box(blake3::hash(black_box(&[7_u8; 1024])));
    });
    measure("typed_content_fingerprint_1k", 100_000, || {
        black_box(Fingerprint::from_bytes(
            FingerprintKind::Content,
            black_box(&[7_u8; 1024]),
        ));
    });

    let schema = json!({
        "type": "object",
        "properties": {"value": {"type": "integer"}},
        "required": ["value"],
        "additionalProperties": false
    });
    let contract_definition = ContractDefinition {
        id: ContractId::new("bench.payload").expect("contract id"),
        version: ContractVersion::new(1, 0, 0),
        owner: "bench".into(),
        schema: schema.clone(),
    };
    let contract = ValidatedContract::compile(contract_definition.clone()).expect("contract");
    let instance = json!({"value": 42});
    measure("contract_validate", 20_000, || {
        contract
            .validate(black_box(&instance))
            .expect("valid instance");
    });
    measure("contract_compile", 2_000, || {
        black_box(ValidatedContract::compile(contract_definition.clone()).expect("compile"));
    });

    let registry = CapabilityRegistry::new();
    registry
        .register(CapabilityDescriptor {
            id: "bench.local".into(),
            capability_id: "bench.route".into(),
            version: "1.0.0".into(),
            contract_id: "bench.payload".into(),
            evidence: EvidenceLevel::Declared,
            health: HealthState::Unknown,
            health_observed_at_ms: 0,
            health_max_age_ms: 10_000,
            privacy: PrivacyClass::Internal,
            side_effect: SideEffectClass::Pure,
            deterministic: true,
            quality_basis_points: 9_000,
            latency_ms: 10,
            token_cost: 0,
            monetary_cost_micros: 0,
            locality: Locality::Native,
        })
        .expect("capability registration");
    let snapshot = registry.snapshot().expect("snapshot");
    let request = ResolutionRequest {
        capability_id: "bench.route".into(),
        max_privacy: PrivacyClass::Internal,
        max_side_effect: SideEffectClass::Pure,
        max_latency_ms: 1_000,
        max_tokens: 1_000,
        max_cost_micros: 1_000,
        require_determinism: true,
        allow_remote: false,
        now_ms: 101,
        minimum_evidence: EvidenceLevel::Declared,
    };
    measure("capability_resolution_one_candidate", 100_000, || {
        black_box(resolve(&snapshot, &request));
    });
    measure("capability_registration", 2_000, || {
        let registry = CapabilityRegistry::new();
        registry
            .register(CapabilityDescriptor {
                id: "bench.local".into(),
                capability_id: "bench.route".into(),
                version: "1.0.0".into(),
                contract_id: "bench.payload".into(),
                evidence: EvidenceLevel::Declared,
                health: HealthState::Unknown,
                health_observed_at_ms: 0,
                health_max_age_ms: 10_000,
                privacy: PrivacyClass::Internal,
                side_effect: SideEffectClass::Pure,
                deterministic: true,
                quality_basis_points: 9_000,
                latency_ms: 10,
                token_cost: 0,
                monetary_cost_micros: 0,
                locality: Locality::Native,
            })
            .expect("register capability");
        black_box(registry.snapshot().expect("snapshot"));
    });

    for node_count in [100_usize, 1_000, 10_000] {
        let mut graph = CausalityGraph::new();
        for index in 0..node_count {
            graph
                .add_node(GraphNode {
                    id: format!("module.{index}"),
                    kind: if index + 1 == node_count {
                        NodeKind::Test
                    } else {
                        NodeKind::Module
                    },
                })
                .expect("graph node");
        }
        for index in 0..node_count.saturating_sub(1) {
            graph
                .add_edge(GraphEdge {
                    from: format!("module.{index}"),
                    to: format!("module.{}", index + 1),
                    kind: EdgeKind::Requires,
                    propagation: [ChangeKind::Implementation].into_iter().collect(),
                    confidence: EdgeConfidence::Proven,
                })
                .expect("graph edge");
        }
        let graph_snapshot = graph.snapshot().expect("graph snapshot");
        let iterations = match node_count {
            100 => 4_000,
            1_000 => 500,
            _ => 100,
        };
        measure(
            &format!("change_cone_{node_count}_node_chain"),
            iterations,
            || {
                black_box(
                    graph_snapshot
                        .compute_change_cone(&["module.0".into()], ChangeKind::Implementation),
                );
            },
        );
    }

    for owner_count in [100, 1_000, 10_000] {
        let mut samples = Vec::new();
        for _ in 0..5 {
            let scheduler = FairScheduler::new([(Lane::Normal, owner_count * 2, 2)])
                .expect("bounded scheduler");
            for owner in 0..owner_count {
                scheduler
                    .enqueue(format!("owner.{owner}"), Lane::Normal, 1_u8)
                    .expect("first owner task");
                scheduler
                    .enqueue(format!("owner.{owner}"), Lane::Normal, 2_u8)
                    .expect("second owner task");
            }
            let started = Instant::now();
            for _ in 0..owner_count * 2 {
                black_box(scheduler.next(Lane::Normal).expect("fair scheduler task"));
            }
            samples.push(started.elapsed());
        }
        report_samples(
            &format!("scheduler_owner_rotation_{owner_count}"),
            owner_count * 2,
            samples,
        );
    }

    let governor = ResourceGovernor::new(ResourceVector {
        cpu_millis: 100,
        memory_bytes: 1_048_576,
        disk_bytes: 1_048_576,
        network_bytes: 0,
        tokens: 10_000,
        cost_micros: 10_000,
    });
    measure("resource_lease_and_delegation", 50_000, || {
        let lease = governor
            .try_lease(
                "bench",
                ResourceVector {
                    tokens: 100,
                    cost_micros: 100,
                    ..ResourceVector::default()
                },
            )
            .expect("root lease");
        let child = lease
            .delegate(
                "bench.child",
                ResourceVector {
                    tokens: 50,
                    cost_micros: 50,
                    ..ResourceVector::default()
                },
            )
            .expect("child lease");
        black_box(child.remaining());
    });
    let proof_compiler = ProofObligationCompiler;
    let changed_path = "crates/bench.rs";
    let mut proof_causality = CausalityGraph::new();
    proof_causality
        .add_node(GraphNode {
            id: changed_path.into(),
            kind: NodeKind::Module,
        })
        .expect("proof graph seed");
    let base_commit = git_commit("HEAD^");
    let exact_head = git_commit("HEAD");
    let assessment = ChangeAssessment::from_git(
        ".",
        &base_commit,
        &exact_head,
        None,
        Some(proof_causality.snapshot().expect("proof graph snapshot")),
    )
    .expect("exact Git bench change assessment");
    measure("git_exact_change_assessment", 10, || {
        black_box(
            ChangeAssessment::from_git(".", &base_commit, &exact_head, None, None)
                .expect("exact Git assessment"),
        );
    });
    measure("proof_obligation_compile", 20_000, || {
        black_box(proof_compiler.compile(assessment.clone()).expect("compile"));
    });
    let compiled_obligations = proof_compiler
        .compile(assessment.clone())
        .expect("compiled obligations");
    let proof_id = "bench.contract.proof";
    let proof_fingerprint = blake3::hash(b"bench proof").to_hex().to_string();
    let proof_inputs = blake3::hash(b"bench inputs").to_hex().to_string();
    let proof_environment = blake3::hash(b"bench environment").to_hex().to_string();
    let proof_backend_run = "backend.bench.001";
    let proof_obligations = compiled_obligations
        .obligations
        .iter()
        .filter(|obligation| obligation.required_kinds.contains(&ProofKind::Contract))
        .map(|obligation| obligation.obligation_id.clone())
        .collect::<Vec<_>>();
    let proof_node = ProofNode {
        proof_id: proof_id.into(),
        kind: ProofKind::Contract,
        outcome: ProofOutcome::Passed,
        fingerprint: proof_fingerprint.clone(),
        head_sha: assessment.exact_head_sha().into(),
        dependencies: Vec::new(),
        obligation_ids: proof_obligations.clone(),
        executor_session_id: None,
        reviewer_session_id: None,
        metadata: json!({
            "backendRunId": proof_backend_run,
            "changeSetFingerprint": compiled_obligations.change_set_fingerprint.clone(),
            "inputsFingerprint": proof_inputs.clone(),
            "environmentFingerprint": proof_environment.clone(),
        }),
    };
    let key = [0x6a; 32];
    let receipt = ProofBackendSigner::new(key)
        .sign(ProofBackendClaims {
            backend_run_id: proof_backend_run.into(),
            proof_id: proof_id.into(),
            kind: ProofKind::Contract,
            proof_fingerprint,
            git_object_format: assessment.git_object_format().into(),
            exact_head_sha: assessment.exact_head_sha().into(),
            change_set_fingerprint: compiled_obligations.change_set_fingerprint.clone(),
            dependencies: Vec::new(),
            obligation_ids: proof_obligations,
            inputs_fingerprint: proof_inputs,
            environment_fingerprint: proof_environment,
            executor_session_id: None,
            reviewer_session_id: None,
        })
        .expect("proof backend receipt");
    let mut proof_graph = ProofGraph::default();
    proof_graph
        .add_verified(proof_node, receipt, &ProofBackendVerifier::new(key))
        .expect("verified proof node");
    measure("proof_minimal_selection", 20_000, || {
        black_box(
            proof_graph
                .certify(&compiled_obligations)
                .expect("proof selection"),
        );
    });

    let mut health = HealthRegistry::default();
    health
        .register(ProbePolicy {
            check_id: "bench.required".into(),
            required: true,
            maximum_age_ms: 10_000,
            degradation_after_failures: 2,
        })
        .expect("health policy");
    health
        .record(ProbeObservation {
            check_id: "bench.required".into(),
            passed: true,
            observed_at_ms: 100,
            latency_ms: 1,
            failure_code: None,
        })
        .expect("health observation");
    measure("readiness_query_one_check", 100_000, || {
        black_box(health.readiness(101).expect("readiness"));
    });

    let telemetry = HotPathRegistry::new(16).expect("telemetry registry");
    measure("telemetry_observation", 100_000, || {
        telemetry
            .observe_micros("bench.dispatch", 12)
            .expect("metric");
    });

    let ephemeral_bus = EventBus::new([(EventLane::Telemetry, 512)]).expect("ephemeral bus");
    let _receiver = ephemeral_bus
        .subscribe(EventLane::Telemetry)
        .expect("event subscriber");
    measure("ephemeral_event_fanout", 20_000, || {
        ephemeral_bus
            .publish_ephemeral(EventEnvelope {
                event_id: "bench.telemetry".into(),
                contract_id: "bench.event".into(),
                contract_version: "1.0.0".into(),
                class: EventClass::Telemetry,
                lane: EventLane::Telemetry,
                producer: "bench".into(),
                privacy: PrivacyClass::Internal,
                causation_id: None,
                correlation_id: None,
                execution_id: None,
                occurred_at_ms: 101,
                payload: json!({"duration": 12}),
            })
            .expect("ephemeral event");
    });

    let cancellation_parent = CancellationToken::new();
    let cancellation_child = cancellation_parent.child_token();
    measure("cancellation_lineage_check", 100_000, || {
        black_box(cancellation_child.is_cancelled());
    });
    measure("cancellation_parent_to_child", 10_000, || {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        parent.cancel();
        black_box(child.is_cancelled());
    });

    measure("runtime_start_and_shutdown", 100, || {
        KernelRuntime::new(RuntimeConfig::default())
            .expect("kernel runtime")
            .shutdown();
    });

    let cold_boot_root = tempfile::tempdir().expect("boot parent");
    let mut cold_samples = Vec::new();
    for sample in 0..5 {
        let start = Instant::now();
        for index in 0..20 {
            let root = cold_boot_root.path().join(format!("cold-{sample}-{index}"));
            let boot = forge_kernel::boot_native(forge_kernel::NativeBootConfig::new(root))
                .expect("cold native boot");
            black_box(&boot.report.boot_fingerprint);
            drop(boot);
        }
        cold_samples.push(start.elapsed());
    }
    report_samples("cold_native_boot", 20, cold_samples);

    let warm_root = tempfile::tempdir().expect("warm state root");
    let mut warm_samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..20 {
            let boot =
                forge_kernel::boot_native(forge_kernel::NativeBootConfig::new(warm_root.path()))
                    .expect("warm native boot");
            black_box(&boot.report.boot_fingerprint);
            drop(boot);
        }
        warm_samples.push(start.elapsed());
    }
    report_samples("warm_native_boot", 20, warm_samples);

    let usage_root = tempfile::tempdir().expect("resource usage state root");
    let mut usage_config = forge_kernel::NativeBootConfig::new(usage_root.path());
    usage_config.resource_limits = ResourceVector {
        tokens: 1_000,
        cost_micros: 1_000,
        ..Default::default()
    };
    let usage_boot = forge_kernel::boot_native(usage_config).expect("usage runtime boot");
    let usage_lease = usage_boot
        .try_resource_lease(
            "bench.usage",
            ResourceVector {
                tokens: 1_000,
                cost_micros: 1_000,
                ..Default::default()
            },
        )
        .expect("usage budget");

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        let mut usage_samples = Vec::new();
        let mut usage_sequence = 0_u64;
        for sample in 0..5 {
            let start = Instant::now();
            for index in 0..200 {
                let usage_id =
                    blake3::hash(format!("m00-resource-usage-{usage_sequence}").as_bytes())
                        .to_hex()
                        .to_string();
                usage_sequence += 1;
                let record = PersistedResourceUsageRecord {
                    usage_id,
                    pool: ResourceUsagePool::Ordinary,
                    owner: "bench.usage".into(),
                    category: "provider.inference".into(),
                    used: PersistedResourceVector {
                        tokens: 1,
                        cost_micros: 1,
                        ..Default::default()
                    },
                    cache_tokens_reused: 8,
                    attribution: ResourceUsageAttribution {
                        project_id: "bench".into(),
                        work_order_id: "FGE-004-M00".into(),
                        execution_id: format!("bench.sample.{sample}"),
                        capability_id: "forge.ai.inference".into(),
                        provider_id: Some("provider.synthetic".into()),
                    },
                };
                black_box(
                    usage_boot
                        .account_resource_usage(&usage_lease, &record)
                        .await
                        .expect("durable resource accounting"),
                );
                black_box(index);
            }
            let elapsed = start.elapsed();
            report_cardinality_sample(
                "resource_usage_durable",
                sample * 200,
                (sample + 1) * 200,
                200,
                elapsed,
            );
            usage_samples.push(elapsed);
        }
        assert_eq!(usage_boot.consumed_resource_budget().tokens, 1_000);
        report_samples("resource_usage_durable", 200, usage_samples);

        let root = tempfile::tempdir().expect("state root");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("state store");
        let cache_identity = CacheIdentity::new(
            "bench",
            "semantic-proof",
            blake3::hash(b"input").to_hex().to_string(),
            BTreeMap::new(),
            blake3::hash(b"environment").to_hex().to_string(),
        )
        .expect("cache identity");
        cache_put(
            &store,
            &cache_identity,
            "bench",
            100,
            10_000,
            PrivacyClass::Internal,
            json!({"cached": true}),
        )
        .await
        .expect("cache population");
        let mut cache_counters = CacheAccessCounters::default();
        let cache_before = cache_counters;
        let mut cache_samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..2_000 {
                black_box(
                    cache_get_observed(&store, &cache_identity, 101, &mut cache_counters)
                        .await
                        .expect("cache lookup"),
                );
            }
            cache_samples.push(start.elapsed());
        }
        report_samples("semantic_cache_lookup", 2_000, cache_samples);
        println!(
            "cache_evidence identity={} before={:?} after={:?} token_savings=not_measured",
            cache_identity.fingerprint, cache_before, cache_counters
        );

        let mut state_write_samples = Vec::new();
        let mut write_sequence = 0_u64;
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..400 {
                store
                    .put_canonical("bench", "key", &json!({"value": write_sequence}))
                    .await
                    .expect("state write");
                write_sequence += 1;
            }
            state_write_samples.push(start.elapsed());
        }
        report_samples("state_transaction_write", 400, state_write_samples);
        let mut state_read_samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..2_000 {
                black_box(
                    store
                        .get_canonical("bench", "key")
                        .await
                        .expect("state read"),
                );
            }
            state_read_samples.push(start.elapsed());
        }
        report_samples("state_read", 2_000, state_read_samples);

        let event_bus = EventBus::new([(EventLane::DurableDomain, 16)]).expect("event bus");
        let mut event_samples = Vec::new();
        let mut event_sequence = 0_u64;
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..200 {
                let index = event_sequence;
                event_sequence += 1;
                event_bus
                    .publish_durable(
                        &store,
                        EventEnvelope {
                            event_id: format!("bench.event.{index}"),
                            contract_id: "bench.event".into(),
                            contract_version: "1.0.0".into(),
                            class: EventClass::DurableLocal,
                            lane: EventLane::DurableDomain,
                            producer: "bench".into(),
                            privacy: PrivacyClass::Internal,
                            causation_id: None,
                            correlation_id: None,
                            execution_id: None,
                            occurred_at_ms: index,
                            payload: json!({"sequence": index}),
                        },
                    )
                    .await
                    .expect("durable event");
            }
            event_samples.push(start.elapsed());
        }
        report_samples("durable_event_outbox", 200, event_samples);

        // Earlier measurements can outlive the 60-second store owner lease.
        // Use a fresh owner for each timed command sample so one slow sample or
        // unrelated host load cannot expire the benchmark store before later samples.
        drop(store);
        let command_store_root = root.path().to_path_buf();

        let auth_key = [0x5a; 32];
        let host_signer = HostDecisionSigner::new(auth_key);
        let command_bus =
            CommandBus::with_host_decision_verifier(1, HostDecisionVerifier::new(auth_key))
                .expect("command bus");
        command_bus
            .register(
                CommandRegistration {
                    contract: Arc::new(contract),
                    required_permissions: BTreeSet::new(),
                    side_effect: SideEffectClass::LocalMutation,
                    idempotency: IdempotencyMode::CallerKeyed,
                    emits_events: false,
                },
                Arc::new(|_context: CommandContext, payload: Value| {
                    Box::pin(async move {
                        Ok(HandlerOutput {
                            value: payload,
                            commit_evidence_fingerprint: Some(
                                blake3::hash(b"command.committed").to_hex().to_string(),
                            ),
                            pending_events: Vec::new(),
                            state_update: None,
                        })
                    })
                }),
            )
            .expect("register command");
        command_bus.seal();
        let contract_id = ContractId::new("bench.payload").expect("contract id");
        let fingerprint = |name: &str| blake3::hash(name.as_bytes()).to_hex().to_string();
        let mut command_samples = Vec::new();
        let mut command_sequence = 0_u64;
        for sample in 0..5 {
            let store = ForgeStateStore::open(&command_store_root)
                .await
                .expect("command benchmark state store");
            let start = Instant::now();
            for _ in 0..200 {
                let index = command_sequence;
                command_sequence += 1;
                let request = CommandRequest {
                    command_id: format!("bench.command.{index}"),
                    principal_id: "bench".into(),
                    authorization_scope: "scope.bench".into(),
                    resource_id: "bench.resource".into(),
                    run_id: "bench.run".into(),
                    contract_id: contract_id.clone(),
                    contract_version: ContractVersion::new(1, 0, 0),
                    idempotency_key: Some(format!("bench.key.{index}")),
                    config_fingerprint: fingerprint("config"),
                    capability_snapshot_fingerprint: fingerprint("capabilities"),
                    dependency_graph_fingerprint: fingerprint("graph"),
                    toolchain_fingerprint: fingerprint("toolchain"),
                    logical_time_ms: index,
                    deterministic_seed: index,
                    sensitivity: SensitivityClass::Public,
                    state_guard: None,
                    payload: json!({"value": index}),
                };
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_millis() as u64;
                let authorization = host_signer
                    .sign(HostAuthorizationClaims {
                        decision_id: format!("bench.decision.{index}"),
                        subject: request.principal_id.clone(),
                        action: request.contract_id.to_string(),
                        scope: request.authorization_scope.clone(),
                        resource_id: request.resource_id.clone(),
                        run_id: request.run_id.clone(),
                        valid_from_ms: now_ms.saturating_sub(1_000),
                        expires_at_ms: now_ms.saturating_add(60_000),
                        permissions: BTreeSet::new(),
                    })
                    .expect("host authorization decision");
                black_box(
                    command_bus
                        .execute(
                            &store,
                            request,
                            &authorization,
                            CancellationToken::new(),
                            Deadline::after(Duration::from_secs(10)),
                            None,
                        )
                        .await
                        .expect("command dispatch"),
                );
            }
            command_samples.push(start.elapsed());
            println!("benchmark_progress=command_dispatch completed_sample={sample}");
            drop(store);
        }
        report_samples("command_dispatch_local_mutation", 200, command_samples);

        let backup_store = ForgeStateStore::open(&command_store_root)
            .await
            .expect("backup benchmark state store");
        let backup_root = tempfile::tempdir().expect("backup root");
        let mut backup_samples = Vec::new();
        for sample in 0..5 {
            let start = Instant::now();
            backup_store
                .backup_to(backup_root.path().join(format!("snapshot-{sample}")))
                .await
                .expect("state backup");
            backup_samples.push(start.elapsed());
        }
        report_samples("state_backup_snapshot", 1, backup_samples);
        let restore_parent = tempfile::tempdir().expect("restore parent");
        let mut restore_samples = Vec::new();
        for sample in 0..5 {
            let start = Instant::now();
            for index in 0..4 {
                let restore_root = restore_parent
                    .path()
                    .join(format!("restore-{sample}-{index}"));
                let restored = ForgeStateStore::restore_from_backup(
                    backup_root.path().join("snapshot-0"),
                    restore_root,
                )
                .await
                .expect("state restore");
                restored
                    .integrity_check()
                    .await
                    .expect("restored integrity");
                drop(restored);
            }
            restore_samples.push(start.elapsed());
        }
        report_samples("state_backup_restore", 4, restore_samples);
    });
}

fn git_commit(revision: &str) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--end-of-options", revision])
        .output()
        .expect("invoke Git to resolve a benchmark commit");
    assert!(output.status.success(), "Git failed to resolve {revision}");
    String::from_utf8(output.stdout)
        .expect("Git commit ID is UTF-8")
        .trim()
        .to_owned()
}

fn measure(name: &str, iterations: usize, mut operation: impl FnMut()) {
    let sample_count = 5;
    let iterations_per_sample = iterations.div_ceil(sample_count).max(1);
    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let start = Instant::now();
        for _ in 0..iterations_per_sample {
            operation();
        }
        samples.push(start.elapsed());
    }
    report_samples(name, iterations_per_sample, samples);
}

fn report_samples(name: &str, iterations_per_sample: usize, samples: Vec<Duration>) {
    let mut nanos_per_operation = samples
        .iter()
        .map(|elapsed| elapsed.as_nanos() / iterations_per_sample.max(1) as u128)
        .collect::<Vec<_>>();
    nanos_per_operation.sort_unstable();
    let median = nanos_per_operation[nanos_per_operation.len() / 2];
    let min = nanos_per_operation[0];
    let max = *nanos_per_operation.last().expect("samples exist");
    println!(
        "benchmark={name} samples={} iterations_per_sample={iterations_per_sample} median_ns_per_operation={median} min_ns_per_operation={min} max_ns_per_operation={max}",
        nanos_per_operation.len()
    );
}

fn report_cardinality_sample(
    name: &str,
    cardinality_start: usize,
    cardinality_end: usize,
    operations: usize,
    elapsed: Duration,
) {
    let elapsed_nanos = elapsed.as_nanos();
    println!(
        "benchmark_sample={name} cardinality_start={cardinality_start} cardinality_end={cardinality_end} operations={operations} elapsed_ns={elapsed_nanos} ns_per_operation={}",
        elapsed_nanos / operations.max(1) as u128
    );
}
