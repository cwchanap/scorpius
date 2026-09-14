# Bevy E2E critical-flow integration plan

## Goal

Integrate `cwchanap/bevy-e2e` into Scorpius and add one rendered, out-of-process smoke test that proves the shipped game can boot, accept real UI input, enter Mission 1, and execute a real battle interaction.

Keep this as one PR. The existing headless domain/presentation suites remain the source of truth for detailed rules; E2E only covers the seams they cannot exercise: the real executable, renderer/window, Bevy picking, campaign screen transitions, and production UI wiring.

## Locked upstream

Pin `bevy_e2e` to:

- repository: `https://github.com/cwchanap/bevy-e2e`
- revision: `13f5d331ade246d1248d6ce6ffdb80e65e5765d7`
- Bevy line: 0.19.x

Do not float against upstream `main` in this PR.

## Scope

### 1. Feature-gate the E2E runtime

Update `Cargo.toml`:

- add `e2e = ["dep:bevy_e2e"]`;
- add optional `bevy_e2e` runtime dependency with `default-features = false, features = ["runtime"]` at the pinned revision;
- add the same pinned package as a dev dependency for the client-side harness.

In `src/lib.rs`, keep the production application shape unchanged and add `BevyE2EPlugin` only under `#[cfg(feature = "e2e")]` when constructing the top-level `App`.

Do not add `RemoteHttpPlugin` separately; `BevyE2EPlugin` owns the harness-selected BRP transport.

### 2. Add only the selectors needed by the critical flow

Use `bevy_e2e::E2eId` directly on existing production entities. Do not add a selector registry or a parallel E2E UI tree.

Start with these stable IDs:

- `campaign.new_game`
- `campaign.skip_dialogue`
- `campaign.start_mission`
- `battle.hud`
- `battle.unit.vanguard`
- `battle.action.move`
- `battle.cell.4.8`

Implementation shape:

- campaign buttons: assign IDs beside the existing `CampaignUiAction` when the shared button helpers create the real button entity;
- battle unit: put the selector on the existing pickable Vanguard `TokenCard` entity;
- destination cell: put the selector on the existing `CellVisual(GridPos::new(4, 8))`. The cell may remain `Pickable::IGNORE`; the E2E click only needs its rendered center so the existing stage-level analytical picking receives the real pointer event;
- HUD/action button: mark the existing production entities rather than adding test-only controls.

Keep all `E2eId` insertion behind `#[cfg(feature = "e2e")]` so normal builds do not depend on the optional crate.

### 3. Add one critical E2E test

Add `tests/e2e.rs` with one serialized happy-path test, approximately:

1. launch `cargo_bin!("scorpius")` through `bevy_e2e::run`;
2. wait for `campaign.new_game`, then click it;
3. wait for and click `campaign.skip_dialogue`;
4. wait for and click `campaign.start_mission`;
5. wait for `battle.hud` and `battle.unit.vanguard`;
6. click Vanguard;
7. click `battle.action.move`;
8. click `battle.cell.4.8`;
9. wait for playback to settle and assert an observable post-move presentation state through BRP inspection (prefer the existing token/UI component state; do not add a custom BRP method or test-only gameplay resource).

The final assertion must prove the move was accepted, not merely that mouse events were sent. Prefer inspecting the existing Vanguard token's rendered component/position or the existing battle-menu visibility/state after the successful move returns to Inspect mode. Pin the simplest stable reflected value discovered during implementation.

This one flow intentionally covers:

`binary boot -> Title -> New Game -> VN -> Skip -> Briefing -> Start Mission -> Battle -> unit selection -> Move -> board click`

Do not duplicate attack resolution, victory, upgrade, persistence, or all seven missions in E2E; those rules already have lower-level coverage.

### 4. Isolate campaign persistence

The E2E child must never use the developer/runner's real Scorpius save.

Use `E2eLaunchOptions::env` to point the child at a unique temporary platform data root created with `std::env::temp_dir()`:

- Linux: `XDG_DATA_HOME`
- macOS: `HOME`
- Windows: `APPDATA`

Use only stdlib path/process/time helpers; do not add `tempfile` just for this test. Best-effort remove the temporary directory after the run.

No save schema change or production save abstraction is needed.

### 5. Add one rendered CI gate

Keep the existing headless coverage job unchanged. Add a separate Linux rendered E2E job in `.github/workflows/ci.yml`:

- install Xvfb plus the renderer/window native packages required by Bevy;
- run the single E2E suite serialized:

```bash
xvfb-run -a cargo test --features e2e --test e2e -- --test-threads=1
```

- on failure, upload `test_output/**` with `actions/upload-artifact@v4` so `bevy-e2e` screenshots, world snapshots, stdout, stderr, and failure metadata are available.

Do not add a Windows matrix in this first consumer PR. Linux/Xvfb is enough to establish the Scorpius gate; broaden only after the first integration is stable.

## Expected files

- `Cargo.toml`
- `Cargo.lock`
- `src/lib.rs`
- selected existing presentation files that spawn the campaign buttons, HUD, token, action button, and board cell
- `tests/e2e.rs`
- `.github/workflows/ci.yml`
- this plan

## Verification

Local/headless regressions:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Rendered E2E on Linux:

```bash
xvfb-run -a cargo test --features e2e --test e2e -- --test-threads=1
```

Final acceptance:

- normal Scorpius builds contain no active E2E runtime unless `e2e` is enabled;
- E2E runs against the real `scorpius` child process and production UI tree;
- the child uses an isolated save root;
- the critical flow reaches Mission 1 and successfully moves Vanguard to the authored valid destination `(4, 8)`;
- failure automatically leaves useful `bevy-e2e` diagnostics in CI;
- existing headless tests and release build remain green.

## Out of scope

No full-campaign E2E, attack/victory E2E matrix, screenshot golden comparison, visual-diff framework, custom BRP methods, generic selector DSL, new gameplay test API, second save layer, extra test crate/workspace, Windows/macOS CI matrix, or second PR.
