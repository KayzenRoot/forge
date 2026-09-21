# Local GEF + HIVE Setup for Forge

## GEF
Use the stable source workspace:
1. clone KayzenRoot/gef-bootstrap;
2. checkout v1.0.0;
3. run npm ci;
4. run npm run validate;
5. optionally run npm audit --audit-level=high.

Do not use an unpublished global GEF CLI.

## HIVE
1. Choose the host project root already configured as HIVE_PROJECTS_ROOT.
2. Clone KayzenRoot/forge beneath that root.
3. Start HIVE using its canonical installation procedure.
4. Register Forge in the HIVE Project Registry using its POSIX-relative path beneath HIVE_PROJECTS_ROOT.
5. Re-inspect it and require state READY.
6. Record Forge Git branch/head and HIVE project identity in an Evidence Bundle.

HIVE receives only the configured project-root boundary. Do not broaden filesystem access for convenience.

## Failure behavior
OFFLINE, DEGRADED or BLOCKED HIVE state does not authorize guessing. Use repository canonical sources and resolve the HIVE condition before claiming live integration.
