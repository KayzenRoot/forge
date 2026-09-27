# Proposed checkpoint delta — FGE-004-M00

Status: PROPOSED ONLY; DO NOT PROMOTE.

Suggested next checkpoint text after independent audit and a separately governed merge:

> FGE-004-M00 has a Rust workspace implementation candidate for S01-S22. Its exact-head hosted validation and outbound-network-blocked doctor checks passed for the implementation candidate; final product certification requires proof obligations, Evidence Bundle, CEC, security/performance evidence and independent audit bound to the merged commit. Product authorization remains bounded by the next admitted Work Order.

This delta cannot be applied now. The current implementation candidate is `0ce565eb217ea3dd1d38e31bf16bdecf772c4175` (tree `27e4edd886a8cadf802db1359bef45fe265c920c`); exact-head Actions run `36278984106` passed all four configured jobs, including Ubuntu and Windows outbound-network-blocked doctor checks. Its 72-entry raw-Git-blob source inventory verified with zero mismatches. A ten-pair performance rerun passed 16 comparable workloads and three scaling checks, while the earlier ten-pair run on the same head failed only resource-lease/delegation. Both reports and the measured overhead are preserved in `docs/PERFORMANCE-M00-CANDIDATE.md`; these measurements are candidate evidence, not approved SLOs. These CI and performance results apply to the implementation SHA only; a later evidence-only commit needs its own exact-head check binding through PR #9. The pre-CD3 UADS digest is not evidence for CD3/CD4. Four fresh security, performance, reliability and certification-systems reviews of one frozen package digest are still pending; checkpoint authority approval is absent. Other M00 certification gaps recorded in the CEC remain in force. Durable live token/cost attribution and its accounting atomicity correction are implemented and locally tested; provider billing integration remains unavailable and budgets stay zero until explicitly configured. `.engineering/CHECKPOINT.md` and `.engineering/CHECKPOINT.json` are intentionally not changed.


C03 historical correction snapshot: candidate `fda8969097320cdad9b8f92dffb4fddd8fc2424e`, baseline `2f95efd4a05ade63dd3e44f3a202c0afe0a46bc4`, performance report SHA-256 `3ca0a256e7c08318706cf5f190acfe31150290cf66676e9958494eaf47b3e217`. Its first five-pair run is preserved. The current `0ce565e` performance reports and their variance are recorded in the current implementation overlay above. This proposal remains unapplied; the canonical checkpoint files stay unchanged until a separately authorized audit disposition, merge, and checkpoint promotion.

## C03 final correction evidence overlay — implementation head f03da4a (2026-09-26)

The corrected implementation commit is `f03da4aa02e619b5affcff09baf6fd56337a286e`. Its local
workspace suite passed (1 CLI, 6 contract, 85 kernel, 26 state, one compile-fail doctest); release
doctor and six S21 local readiness cases pass at schema 7. Local egress blocking is not claimed.
Performance passed 13/13 comparable workloads using the fixed 20% baseline allowance and all three
scaling checks. These are candidate results only. The exact performance JSON is bound by SHA-256
`4b068854c60575b0a975b8495c956005194070ea381a1be52a44070fbf523b7f`.

This proposed text remains unapplied. The final documentation/evidence head requires its own raw
source-fingerprint verification and exact-head hosted CI through PR #9, followed by four fresh,
distinct reviews of one frozen package digest. This overlay does not promote or certify M00, merge the
PR, change either canonical checkpoint file, or begin M01.

## C03 reviewer-correction evidence overlay — e1da083 (2026-09-27)

The implementation evidence now includes the R1 SQLite writer-contention correction and the P1/P2
repeatable performance and candidate-reference gate. At code commit
`e1da083893d9335118519392c1cdb36021cbfc74`, local source verification passed for 73 raw Git blob
entries; the full local validation suite, six S21 local groups, 13/13 comparable performance checks,
19/19 reference gates, three scaling checks, and paired idle-memory budget passed. Local egress was
not asserted. The measurement reports and raw evidence are preserved in
`C03-CORRECTION/`.

This is still candidate evidence only. A new evidence/documentation commit needs its own exact-head
hosted checks and a replacement frozen package with a matching current context lock. Four fresh
independent package-bound reviews and applicable audit disposition remain required. This proposal is
not applied: do not merge PR #9, edit or promote either canonical checkpoint, certify M00, or start
M01.
