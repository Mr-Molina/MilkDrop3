# Task 0.3 report
Files: .github/workflows/ci.yml, deny.toml, scripts/check-no-native-build.sh (mode 100755).
- cargo-deny 0.20.2 installed; `cargo deny check` on repo: advisories/bans/licenses/sources ok (warnings: duplicate miniz_oxide, unused cc wrappers since winit is not yet consumed).
- Script: loops cc/cmake/bindgen with `cargo tree --workspace --target all -e normal,build -i X --depth 1 --prefix none`; "did not match" = absent = ok; any parent other than wayland-backend/android-activity (cc only) fails. Feature check via `-e features -i` for wayland-backend `log` and android-activity `game-activity`. Also finds *.c/cc/cpp/h/hpp outside code/, target/, .git/.
- Validated in a scratch workspace depending on winit 0.30.13: clean passes; wayland-backend log, android-activity game-activity, direct cc, cmake, and a stray .c each fail. cargo deny bans passes with winit, fails with direct cc.
- ci.yml YAML parses (python yaml); not runnable here. MD_REQUIRE_GPU/WGPU_ADAPTER_NAME set only on ubuntu via matrix expression; checkout lfs: true; libpulse-dev + mesa-vulkan-drivers installed. Uses actions/checkout@v4, Swatinem/rust-cache@v2, EmbarkStudios/cargo-deny-action@v2.
- Concerns: ci.yml also installs libvulkan1/pkg-config (extra); no libasound2-dev (cpal is to be windows-gated per F8, not yet added). No .gitattributes yet (LFS arrives with corpus task).

## Fix round 1
Removed the job-level env (empty strings on macOS/Windows). Added a step with `if: runner.os == 'Linux'` appending MD_REQUIRE_GPU=1 and WGPU_ADAPTER_NAME=llvmpipe to $GITHUB_ENV, so they are absent elsewhere. YAML re-parsed OK with python yaml.
