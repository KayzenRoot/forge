# HIVE Integration Contract

## Role
HIVE provides project registration, context, memory, retrieval and governed task-context capabilities. Forge remains operable from Git without HIVE.

## Current upstream baseline
Repository: KayzenRoot/hive  
Observed main SHA at bootstrap: 736633504d6d68543bd6c69d59bba6adf8098cd5

## Local integration path
1. Clone Forge beneath the host directory configured as `HIVE_PROJECTS_ROOT`.
2. Start HIVE according to its own installation contract.
3. Register Forge through HIVE Project Registry using the relative path beneath that root.
4. Inspect and confirm READY before relying on HIVE context features.
5. Never grant HIVE a broader filesystem boundary than required.

## Failure policy
If HIVE is OFFLINE/DEGRADED/BLOCKED, execution falls back to canonical repository sources. Never invent missing context or promote a checkpoint from stale HIVE state.

Live registration is intentionally NOT claimed by this bootstrap because it requires the user's local HIVE runtime and filesystem.
