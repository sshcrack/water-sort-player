# Workspace Instructions

This repository is a Rust workspace for automating and solving Water Sort levels. Treat the codebase as a multi-crate system with clear boundaries between core puzzle logic, device I/O, capture, and the main player loop.

## Project Shape

- `crates/water-sort-core` contains shared domain types, layout data, and constants.
- `crates/water-sort-device` owns platform-specific capture/input backends for Windows and Linux.
- `crates/water-sort-solver` contains the A* solver and discovery logic.
- `crates/water-sort-capture` handles image capture and test data collection.
- `crates/water-sort-player` is the main binary and orchestration layer.
- `crates/water-sort-image-debugger` is a utility binary for image analysis.

## Working Rules

- Preserve feature gates and platform-specific `cfg` blocks unless the change explicitly requires touching them.
- Keep shared logic in the lower-level crates; avoid moving solver, capture, or layout logic into the player crate.
- Prefer small, focused edits that match the existing style and avoid unrelated formatting churn.
- Use the existing workspace dependency versions from the root `Cargo.toml` instead of adding ad hoc versions in individual crates.
- If a file is getting too large or complex, consider if some of its logic can be extracted into a new module or crate, but avoid doing this unless it's necessary for the change at hand.
- You may interface with the Android emulator and the game as needed to understand behavior, but avoid making manual changes to the emulator state or game state outside of automated test fixtures.

## Build And Test

- Build with `cargo build`.
- Run tests with `cargo nextest r -r --workspace`.
- The CI workflow installs OpenCV and related system packages on Ubuntu before building and testing.
- The default test profile disables the solver visualization overhead.
- An android emulator is connected right now. You can run the main binary with `cargo run` as a final check if the implementation is working properly.

## Feature Flags And Runtime Notes

- `solver-visualization` is enabled by default for the player crate.
- `discovery-debugging` enables interactive discovery stepping.
- `collect-test-data` enables capture-driven test data collection.
- The main binary supports `--quick`, which skips launching the app and start-button automation.

## Conventions And Pitfalls

- OpenCV uses BGR channel order; color helpers in `water-sort-core::constants` already account for that.
- `water-sort-device` is the abstraction layer for screen capture and input. Keep scrcpy and platform handling isolated there.
- Tests in `crates/water-sort-player/src/level_tests.rs` rely on captured images and generated fixtures; missing assets can cause tests to skip.
- `build.rs` exists in both `water-sort-core` and `water-sort-player`, so changes to assets or generated data may need a rebuild.
- The solver and discovery flow are tightly coupled to bottle layout detection and move execution, so validate both when changing puzzle logic.

## Read First

- [crates/water-sort-player/src/main.rs](../crates/water-sort-player/src/main.rs)
- [crates/water-sort-player/src/app.rs](../crates/water-sort-player/src/app.rs)
- [crates/water-sort-solver/src/lib.rs](../crates/water-sort-solver/src/lib.rs)
- [crates/water-sort-solver/src/discovery.rs](../crates/water-sort-solver/src/discovery.rs)
- [crates/water-sort-core/src/constants.rs](../crates/water-sort-core/src/constants.rs)
- [crates/water-sort-device/src/lib.rs](../crates/water-sort-device/src/lib.rs)
- [crates/water-sort-player/src/level_tests.rs](../crates/water-sort-player/src/level_tests.rs)

## Documentation

There is no repo-level documentation file yet. If you add one later, link to it from here instead of duplicating the same guidance.
