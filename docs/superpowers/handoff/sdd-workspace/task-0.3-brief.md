### Task 0.3: CI and dependency policy

**Files:**
- Create: `.github/workflows/ci.yml`, `deny.toml`, `scripts/check-no-native-build.sh`

- [ ] **Step 1:** Write `deny.toml`: allowed licenses from Global Constraints; `[bans]` denies `cc`, `cmake`, `bindgen` as build dependencies anywhere in the graph (`wildcards = "deny"` is not needed; use `deny = [{ name = "cc" }, { name = "cmake" }]` and `multiple-versions = "warn"`).
- [ ] **Step 2:** Write `scripts/check-no-native-build.sh`: fails if `cargo tree -e build --workspace` lists `cc` or `cmake`, or if any file matching `*.c`, `*.cc`, `*.cpp`, `*.h`, `*.hpp` exists outside `code/` (the `code/` exemption is removed in Task 9.1).
- [ ] **Step 3:** Write `ci.yml`: matrix ubuntu-latest, macos-latest, windows-latest; steps: toolchain from `rust-toolchain.toml`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `scripts/check-no-native-build.sh`, `cargo test --workspace`. Ubuntu installs `mesa-vulkan-drivers` and sets `WGPU_ADAPTER_NAME=llvmpipe` for GPU tests (used from Milestone 5).
- [ ] **Step 4:** Run the script locally: `scripts/check-no-native-build.sh`. Expected: exit 0.
- [ ] **Step 5:** Commit: `ci: add matrix build, cargo-deny, and native-code guard`

