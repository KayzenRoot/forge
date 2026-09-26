# Security

Baseline risk is STANDARD; security-sensitive, signing, privileged authentication, money or irreversible operations escalate to HIGH_ASSURANCE.

Never commit secrets. External integrations use least privilege. Inputs crossing trust boundaries require validation. Dependencies and CI actions must be pinned/controlled. Web3 signing keys, production credentials and destructive deployment capabilities are never stored in HIVE context or repository plaintext.

Threat modeling and stronger proof obligations become mandatory before security-critical implementation.

The M00 source fingerprint verifier treats Git trees and manifests as untrusted input. It uses
shell-free Git arguments, disables lazy fetch and environment overrides, bounds command time and
pipe output, and caps trees at 100,000 entries/16 MiB, manifests at 1 MiB/4,096 rows, paths at
4,096 bytes/64 components, each source blob at 16 MiB, and a source batch at 128 MiB. It rejects
symlinks, submodules/non-regular entries, ambiguous path aliases, unavailable blobs, traversal,
and Git object IDs that do not match `git rev-parse --show-object-format` (SHA-1: 40 hex; SHA-256:
64 hex). Resource exhaustion fails with a typed `resource_limit` result.

Before ordinary state, evidence, outbox, receipt, resource-usage, or cache payloads are persisted,
the state store rejects secret-bearing field names (including token/key/secret suffixes) and
credential-shaped values. Errors contain no rejected value. Emergency runtime records accept only
bounded stable event codes, never arbitrary caller text. Secrets stay in secure references and are
excluded from canonical rows, replay records, cache, and backups.
