# HPA-632 GitHub CI Closeout Design

## Context

HPA-632's retained Mission 1 implementation and product validation are already present on `main`. The validation ledger records both product questions as **YES** and all local gates as passing. The remaining unchecked acceptance criterion is the GitHub Actions CI gate.

The first real GitHub Actions run on `main` reached `cargo fmt --check`, then failed while compiling Bevy during Clippy because `wayland-sys` could not find the Ubuntu `wayland-client` development package through `pkg-config`. This is runner provisioning, not a Scorpius combat or presentation defect.

HPA-635 remains blocked by HPA-632 until this external acceptance gate is green.

## Goal

Close HPA-632 with the smallest runner-only fix that lets the existing four-command CI contract execute on GitHub-hosted Ubuntu without changing the locked game stack or runtime feature surface.

## Decision

Keep the current Rust 2024 / Bevy `0.19` application unchanged and provision Bevy's documented Ubuntu build dependencies in `.github/workflows/ci.yml` before the Rust checks run.

Install:

```text
g++
pkg-config
libx11-dev
libasound2-dev
libudev-dev
libxkbcommon-x11-0
libwayland-dev
libxkbcommon-dev
```

Then run the existing contract unchanged:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-targets`
4. `cargo build --release`

The workflow should continue using `ubuntu-latest` and the current `bevy = "0.19"` dependency with default features.

## Why this approach

### Chosen: provision Linux system packages

This matches Bevy's documented Linux prerequisites and fixes the environment that GitHub Actions currently lacks. It preserves the application configuration already validated on macOS and still gives the project a Linux compile/test signal.

### Rejected: disable Bevy default features to avoid Wayland/audio dependencies

That would change the runtime feature surface solely to satisfy CI. HPA-632 explicitly validated the current Bevy application shape, and there is no product reason to narrow platform/window/audio features now.

### Rejected: move CI to a macOS runner

That would hide the Linux dependency problem rather than make the checked-in workflow portable, while using a more expensive/slower runner for a hobby project. The current Ubuntu runner is sufficient once its documented native dependencies are installed.

## Scope

Modify only:

- `.github/workflows/ci.yml` — install documented Bevy Ubuntu build dependencies before Rust validation.
- `docs/validation/hpa-632.md` — replace the stale "no GitHub Actions run" note with the actual failed-run diagnosis and, after verification, the green-run evidence.

Do not modify combat rules, Mission 1 content, presentation, assets, `Cargo.toml`, or `Cargo.lock` unless the provisioned runner reaches Rust compilation and exposes a genuine source-level failure. If that happens, fix only the concrete failure and record it in the validation ledger.

## Verification flow

Use the existing failed run as the red baseline:

- workflow run `32696452371`
- `cargo fmt --check`: passed
- Clippy: failed before compiling Scorpius because `wayland-client.pc` was missing
- tests/release build: skipped

After provisioning packages, the branch CI must reach and pass all four existing Rust commands. No new matrix, cache layer, cross-platform job, or reusable workflow is needed.

## Acceptance

HPA-632 is ready to close when:

- [ ] GitHub Actions installs the documented Bevy Ubuntu dependencies successfully.
- [ ] Clippy reaches Scorpius and passes with `-D warnings`.
- [ ] `cargo test --all-targets` passes in GitHub Actions.
- [ ] `cargo build --release` passes in GitHub Actions.
- [ ] `docs/validation/hpa-632.md` records the successful run and marks the final CI acceptance criterion complete.
- [ ] No product/runtime dependency or feature change was introduced just for CI.

After this gate is green, HPA-632 can move to Done and HPA-635 becomes the next actionable Scorpius product ticket.

## Delivery rule

This is a closeout delta for the existing HPA-632 ticket, not a new subsystem or feature. Keep the branch focused on CI portability and validation evidence only; do not absorb HPA-635 campaign work into this PR.
