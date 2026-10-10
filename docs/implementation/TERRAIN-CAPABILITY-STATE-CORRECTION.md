# Terrain Capability State Correction

The post-M9 reference audit established that `class470.method9712(WorldView)` is the exact public-pinned terrain-builder source oracle.

The target capability state therefore uses `source_verified` rather than `blocked`.

This state does **not** mean the Rust implementation exists. `TERRAIN-004` production implementation and executable differential fixtures remain required before implementation parity may be claimed.

The target profile, profile identity digest, and M9 capability regression are updated together so the capability layer reports the source state truthfully without pretending production support exists.
