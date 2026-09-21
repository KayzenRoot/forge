# GEF Adoption

Forge adopts the stable **GEF Bootstrap v1.0.0** source-workspace release.

Upstream: KayzenRoot/gef-bootstrap  
Release tag: v1.0.0  
Release commit: 866fe3af8cccc65c929aaf6a47a924401fa448b3

GEF is not copied wholesale into Forge. Forge follows its governance contract and keeps project-specific canonical sources here. Local executors should validate a checkout of the pinned GEF source workspace with `npm ci` and `npm run validate` before relying on it.

Any future GEF upgrade is an explicit governed decision, never an implicit pull from upstream main.
