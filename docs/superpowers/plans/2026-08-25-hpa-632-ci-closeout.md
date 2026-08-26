# HPA-632 GitHub CI Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make HPA-632's existing GitHub Actions contract pass on `ubuntu-latest`, record the external evidence, and unblock HPA-635 without changing Scorpius product behavior.

**Architecture:** Keep the validated Rust 2024 / Bevy 0.19 application untouched. Treat the current failed GitHub Actions run as the red test, provision Bevy's documented Ubuntu native dependencies in the existing single CI job, then update the HPA-632 validation ledger only after the same four Rust gates pass remotely.

**Tech Stack:** GitHub Actions, Ubuntu hosted runner, Rust stable, Bevy `0.19`, Cargo.

**Spec:** `docs/superpowers/specs/2026-08-25-hpa-632-ci-closeout-design.md`

## Global Constraints

- Keep `bevy = "0.19"`, Rust edition 2024, `Cargo.lock`, and Bevy default features unchanged.
- Keep one `ubuntu-latest` CI job; do not add a platform matrix, cache framework, reusable workflow, container image, or macOS runner.
- Preserve the existing commands exactly: `cargo fmt --check`, strict Clippy, `cargo test --all-targets`, and `cargo build --release`.
- Install only the documented Linux build dependencies needed by the existing Bevy feature set.
- Do not modify combat, presentation, assets, Mission 1 content, or HPA-635 campaign code unless remote CI reveals a genuine source-level failure after the runner is provisioned.
- Update acceptance evidence only from an actual GitHub Actions run; do not substitute local success for the remaining remote gate.
- This PR is HPA-632 closeout only. HPA-635 remains separate.

## File Map

| Path | Responsibility |
| --- | --- |
| `.github/workflows/ci.yml` | Provision Bevy's Ubuntu system dependencies, then run the unchanged four-command Rust CI contract |
| `docs/validation/hpa-632.md` | Record the failed-run diagnosis, successful closeout run, and final acceptance status |

---

### Task 1: Provision the GitHub Actions runner and prove all four gates execute

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: existing `ubuntu-latest` job and the current Rust/Bevy dependency graph.
- Produces: a runner with the native libraries required for Bevy 0.19 to compile before the existing Cargo checks run.

- [ ] **Step 1: Preserve the failing run as the red baseline**

Use GitHub Actions run `32696452371` as the pre-change failure evidence.

Expected baseline:

```text
cargo fmt --check: PASS
cargo clippy --all-targets --all-features -- -D warnings: FAIL before Scorpius compilation
root cause: wayland-sys cannot find wayland-client.pc
cargo test --all-targets: SKIPPED
cargo build --release: SKIPPED
```

Do not change source code to address this failure; the error is emitted by the Ubuntu native dependency probe.

- [ ] **Step 2: Add one explicit Ubuntu dependency-install step**

Insert this step after checkout and before Rust/Cargo validation:

```yaml
      - name: Install Bevy Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            g++ \
            pkg-config \
            libx11-dev \
            libasound2-dev \
            libudev-dev \
            libxkbcommon-x11-0 \
            libwayland-dev \
            libxkbcommon-dev
```

Keep the existing Rust setup and the four Cargo commands unchanged.

- [ ] **Step 3: Inspect the workflow diff before pushing**

Run locally:

```bash
git diff --check
git diff -- .github/workflows/ci.yml
```

Expected: one new package-install step only; no runner, matrix, Bevy feature, Cargo, source, or test changes.

- [ ] **Step 4: Commit the runner provisioning change**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: install Bevy Linux dependencies"
```

- [ ] **Step 5: Push and inspect the new GitHub Actions run**

Expected remote sequence:

```text
Install Bevy Linux dependencies: PASS
cargo fmt --check: PASS
cargo clippy --all-targets --all-features -- -D warnings: PASS
cargo test --all-targets: PASS
cargo build --release: PASS
```

If package installation fails because a listed Ubuntu package name is unavailable, correct only that package name using the runner's Ubuntu release and rerun. If the runner reaches Rust compilation and exposes a Scorpius warning/test/build failure, treat that as a concrete HPA-632 acceptance defect and fix only the reported issue before rerunning.

---

### Task 2: Close the HPA-632 acceptance ledger from remote evidence

**Files:**
- Modify: `docs/validation/hpa-632.md`

**Interfaces:**
- Consumes: the first green GitHub Actions run from Task 1.
- Produces: a complete HPA-632 acceptance matrix with reproducible remote CI evidence.

- [ ] **Step 1: Replace the stale external-CI note**

Change the acceptance entry that currently says no GitHub Actions run exists. Record:

```text
- the original failed run ID 32696452371 and its missing wayland-client diagnosis
- the successful closeout run ID and commit SHA
- that fmt, strict Clippy, all-target tests, and release build all passed remotely
```

Mark the final CI acceptance checkbox complete only after the successful run exists.

- [ ] **Step 2: Update the validation conclusion without changing product findings**

Preserve the existing product conclusions:

```text
Bevy 2.5D battlefield + native overlay maintainability: YES
Intent manipulation versus pure damage optimization: YES
```

Add one short closeout note that the remaining blocker was GitHub-hosted Ubuntu provisioning and is now resolved. Do not rewrite the already-recorded manual playtest evidence.

- [ ] **Step 3: Verify documentation-only follow-up**

Run locally:

```bash
git diff --check
git diff -- docs/validation/hpa-632.md
```

Expected: only evidence/status text changes; no new product requirement or scope expansion.

- [ ] **Step 4: Commit the closeout evidence**

```bash
git add docs/validation/hpa-632.md
git commit -m "docs: close HPA-632 CI acceptance"
```

- [ ] **Step 5: Require the final branch CI to remain green**

Because the documentation commit retriggers the workflow, verify the branch head again passes:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Only then is HPA-632 ready to move to Done and unblock HPA-635.

---

## Self-Review

- **Spec coverage:** both required changes are mapped: runner provisioning and validation evidence.
- **Scope:** no application architecture, Cargo feature, campaign, persistence, or gameplay work is included.
- **Type/interface consistency:** no Rust interfaces are introduced or changed.
- **No placeholders:** package list, workflow snippet, verification commands, expected failure, and expected success are explicit.
- **Single-PR rule:** all remaining HPA-632 closeout work stays on this branch; HPA-635 is not mixed into it.
