# Changelog

## Unreleased

- **macOS support.** The addon now ships a universal macOS library next to the Linux and Windows ones, so the Rust planner loads on macOS.
- **Release archive.** Each release attaches `GdPlanningAI-X.Y.Z.zip`, the addon with its Linux, macOS and Windows binaries. GitHub's own source archives hold Git LFS pointers in their place.
- **Library version check.** The Rust library reports its version (`GdPAIPlanScheduler.get_engine_version`), and the addon warns at startup when it differs from `plugin.cfg`, which means a stale or mismatched binary.
