# HPA-480 — Native UI visual parity implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Match the supplied Scorpius UI across all eight screen families, implement its missing interactions, and preserve the working seven-mission game.

**Architecture:** Keep the existing domain/campaign state, commands and screen flow. Replace presentation with a fixed-canvas native Bevy UI, flat tactical board, typed snapshots and concrete styling/layout helpers. Adapt existing asset loading, playback and tests instead of adding another runtime or simulation.

**Tech stack:** Rust 2024, Bevy 0.19, existing serde/serde_json and headless Rust tests; an opt-in native capture example and development-only image comparison utility.

**Spec:** [2026-09-05-hpa-480-ui-visual-parity-design.md](../specs/2026-09-05-hpa-480-ui-visual-parity-design.md)

## Global constraints

- One ticket, one branch, one PR; implement and accept on this existing draft PR.
- Rust 2024, Bevy 0.19, one application crate, committed Cargo.lock.
- Native Bevy presentation; no second UI runtime, physics engine, networking, or generic ability/UI framework.
- `src/domain/` remains Bevy-free; `BattleRuntime` and `CampaignRuntime` remain authoritative.
- One 1920 × 1080 logical canvas; uniform scale-to-fit with letterboxing, not responsive mobile reflow.
- Preserve combat balance, authored missions, committed intents, pilot restrictions, RNG order, and current save semantics.
- Full native visual parity is the final gate; unit tests alone cannot satisfy it.
- Existing normal tests remain headless; windowed visual capture is a separate opt-in command.
- No save migration or backward-compatibility layer, new save slots, checkpoints, undo, new missions, inventory, or settings screen.

**Continue on `hpa-480-ui-visual-parity`.** These tasks are internal checkpoints, not more Linear tickets or PRs. Do not merge this documentation-only head. Conventional commits accumulate on this branch; merge the final PR only after implementation and acceptance.

## 0. Baseline and file ownership

Baseline: `d981682840eb9147ba9eb7f7c56b2ceae88a3aed`. Read `CLAUDE.md`, the spec and current files before editing. The new design supersedes the old Camera3d/glTF presentation requirement, not domain rules.

| Path | Responsibility |
| --- | --- |
| `src/presentation/theme.rs` (new) | Concrete palette, typography, panel/button/pip/bar helpers. |
| `src/presentation/layout.rs` (new) | Canvas fit/coordinates, board positions, menu clamping. |
| `src/presentation/assets.rs` | Image/font/glyph handles and readiness; retire glTF-only loading at cutover. |
| `src/presentation/screens/mod.rs`, `title.rs`, `dialogue.rs`, `briefing.rs`, `hangar.rs`, `ending.rs` (new) | Screen-specific layouts; share dialogue for Story/Aftermath. |
| `src/presentation/campaign_ui.rs` | Existing actions, campaign snapshots, cursor, persistence routing and cleanup. |
| `src/presentation/battlefield.rs`, `sync.rs` | Flat grid/tokens/props and synchronization from domain state. |
| `src/presentation/ui.rs` | HUD snapshots/header/inspector/log/threat/preview/result rendering. |
| `src/presentation/battle_menu.rs` (new) | Anchored command, weapon and stance menus. |
| `src/presentation/interaction.rs` | Inspection/target routing, ready navigation, restart, pointer/keyboard parity. |
| `src/presentation/playback.rs` | Ordered event effects/input lock and six-entry recent log. |
| `src/presentation/mod.rs`, `src/app.rs` | Resources, components, lifecycle and explicit system order. |
| `assets/ui/` (new), existing `assets/vn/` | Supplied art and static decorations, not runtime HTML. |
| `tests/ui_layout.rs`, `ui_interaction.rs`, `ui_snapshots.rs` (new) | Deterministic helper, interaction and view-data coverage. |
| Existing presentation/campaign tests and inline domain tests | Adapt visual assertions; retain behavior/persistence coverage. |
| `src/presentation/capture.rs`, `examples/ui_capture.rs` (new, opt-in) | Native capture scenarios using production rendering. |
| `tools/compare_ui.py` (new) | Same-sized side-by-side, overlay and diff images. |
| `docs/references/hpa-480/`, `docs/validation/hpa-480.md` | Provenance, original/aligned/extension references and final evidence. |

No engine crate, plugin suite, database or generic workflow layer is needed. New helper names below are proposed project APIs, not claims about Bevy APIs.

## Task 1 — Reference intake, assets and native layout proof

**Files:** reference directory; new `theme.rs`, `layout.rs`; additive changes to `assets.rs`, `mod.rs`; `tests/ui_layout.rs`; `assets/ui/`. Keep old asset wiring until Task 3 replaces its consumers.

**Inputs:** exact HTML/ZIP in `docs/references/hpa-480/reference-manifest.json`; four existing VN images.
**Outputs:** source/art inventory, theme helpers, presentation asset handles, and these layout contracts:

```rust
pub struct CanvasLayout {
    pub scale: f32,
    pub offset: Vec2,
}
// Associated functions: CanvasLayout::fit(Vec2) -> CanvasLayout;
// CanvasLayout::to_design(Vec2) -> Option<Vec2>.
// Free function: clamp_menu(anchor: Vec2, size: Vec2, bounds: Vec2) -> Vec2.
```

- [ ] Obtain the exact source attachments and verify SHA-256 against the manifest. Transfer PNG references into durable repository/issue storage and link the real location in this PR. The draft has metadata only: a Linear ZIP PUT failed. Remove that availability warning only after successful transfer; do not reconstruct the design from memory.
- [ ] Inspect the JSON-decoded `__bundler/template`, not its loading thumbnail. Inventory hover/selected/disabled states and source measurements. Keep original screenshots separate from new references. The old animation pause instant is unspecified and must be replaced by an explicitly timed capture before judging animated pixels.
- [ ] Extract the five named PNG art resources into `assets/ui/key_art.png`, `briefing.png`, `vanguard.png`, `gunner.png`, `interceptor.png`; verify their recorded hashes. Compare/reuse the four VN images. No demo JavaScript becomes runtime code.

This one-off Python procedure extracts only the requested artwork:

```python
import base64, gzip, json, re
from pathlib import Path
source = Path("Scorpius UI (offline).html").read_text()
def section(name):
    match = re.search(r'<script type="__bundler/' + name + r'">\s*(.*?)\s*</script>', source, re.S)
    if match is None:
        raise ValueError(f"Missing bundle section: {name}")
    return json.loads(match.group(1))
manifest = section("manifest")
resources = {entry["id"]: entry["uuid"] for entry in section("ext_resources")}
outputs = {"keyArt": "key_art", "briefArt": "briefing", "mechVa": "vanguard", "mechGu": "gunner", "mechIn": "interceptor"}
Path("assets/ui").mkdir(parents=True, exist_ok=True)
for name, stem in outputs.items():
    entry = manifest[resources[name]]
    if entry["mime"] != "image/png":
        raise ValueError(f"Unexpected image type for {name}")
    data = base64.b64decode(entry["data"], validate=True)
    if entry.get("compressed"):
        data = gzip.decompress(data)
    Path(f"assets/ui/{stem}.png").write_bytes(data)
```

- [ ] Add failing layout tests; run `cargo test --test ui_layout` before filling in helper behavior. After the initial compile-level failure, confirm that wrong fit/clamp values produce assertion failures rather than merely relying on an absent symbol.

```rust
use bevy::prelude::Vec2;
use scorpius::presentation::layout::{CanvasLayout, clamp_menu};

#[test]
fn letterbox_and_pointer_coordinates_share_the_same_fit() {
    let fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    assert!((fit.scale - 5.0 / 6.0).abs() < 0.00001);
    assert!((fit.offset - Vec2::new(0.0, 50.0)).length() < 0.001);
    assert!(fit.to_design(Vec2::new(800.0, 10.0)).is_none());
    let center = fit.to_design(Vec2::new(800.0, 500.0)).unwrap();
    assert!((center - Vec2::new(960.0, 540.0)).length() < 0.001);
}

#[test]
fn lower_right_menu_stays_inside_board() {
    assert_eq!(
        clamp_menu(Vec2::new(900.0, 900.0), Vec2::new(236.0, 320.0), Vec2::splat(912.0)),
        Vec2::new(676.0, 592.0),
    );
}
```

- [ ] Implement minimum width/height fit, centered offset and no coordinates outside the canvas. A zero/minimized dimension yields no actionable canvas. Clamp menu origin to `[0, bounds - menu_size]`. Test all primary/secondary sizes and edges; convert physical/logical coordinates only at the boundary.
- [ ] Implement theme and required-asset readiness. Match font families/weights/tracking/crops. Validate title typography and one complex card in an actual Bevy window now, including hover/disabled and pulse styling; resolve issues before replicating components.
- [ ] Run `cargo fmt --check`, `cargo test --test ui_layout`, `cargo test --test presentation_app`, `cargo check --all-targets`. Record proof images and asset-error behavior. Commit `feat: add HPA-480 visual foundation and reference assets` on this branch.

## Task 2 — Campaign screens and truthful navigation

**Files:** `campaign_ui.rs`, new `screens/`, `app.rs`, `mod.rs`; `tests/campaign_flow.rs`, `campaign_persistence.rs`, `ui_snapshots.rs`.
**Consumes:** Task 1 helpers/assets; existing `CampaignUiAction`, `CampaignRuntime`, `DialogueCursor`, `CompletionReceipt`.
**Produces:** the six campaign screens (Title, Story, Briefing, Aftermath, Hangar, Ending), `CampaignUiAction::SkipDialogue`, typed view data and unchanged persistence semantics.

- [ ] Add failing fixture tests for Skip at each pre-mission line, duplicate advances at a queued transition, missing/corrupt/completed saves, and aftermath reading the just-completed mission. Assert destination/cursor and serialized campaign state, not only display text.
- [ ] Add the explicit Skip enum arm to existing routing. Callers guard current PreMissionStory and pending transitions; Skip never invokes completion or purchases. Keep it off Aftermath.

```rust
CampaignUiAction::SkipDialogue => {
    next_state.set(GameScreen::Briefing);
}
```

- [ ] Move layout construction only into `screens/`; keep actions and persistence in the existing adapter. Implement the six exact layouts in spec section 5. Share the dialogue builder and preserve all authored portraits/lines.
- [ ] Derive progress/save pips from saved state. Briefing reads current definition/mission data; build a fixed-seed fixture once at entry only when board/enemy metadata is needed, not every frame or through gameplay RNG.
- [ ] Render the persisted receipt and validated purchase results. Failed writes retain previous credits/levels. Inert controls block pass-through. Next Drop preserves next pre-mission story; final Aftermath and completed Continue route to Ending.
- [ ] Add upgrade snapshots using existing costs/caps/effects. Pin Mobility as evasion, and test affordable/unaffordable/purchased/MAX/failure display states. Place success/save-error feedback within the new layout.
- [ ] Run `cargo test --test campaign_flow`, `cargo test --test campaign_persistence`, `cargo test --test campaign_model`, `cargo test --test ui_snapshots`, `cargo check --all-targets`. Capture each screen and purchase/save states. Commit `feat: match campaign screens to the Scorpius reference`.

## Task 3 — Flat tactical rendering, asset cutover and playback

**Files:** `battlefield.rs`, `sync.rs`, `playback.rs`, `assets.rs`, `mod.rs`, `app.rs`; inline visual/asset tests; `tests/presentation_app.rs`, `ui_layout.rs`.
**Consumes:** domain board/unit IDs, event queue/input lock and Task 1 helpers/assets.
**Produces:** one flat production renderer and one presentation asset gate; native-grid effects with retained event ordering.

- [ ] Add failing tests for cells/tokens/terrain/explosives/extraction, full-roster glyph mapping, and one camera/root after re-entry/restart. Replace glTF-only assertions with equivalent visual coverage; retain gameplay tests.
- [ ] Put flat cells and tokens under the HUD's fitted canvas. Use source 96 px cells, 6 px gaps and 86 px tokens, reading board dimensions from state. Add `cell_origin(GridPos) -> Vec2` to `layout.rs`:

```rust
pub fn cell_origin(cell: GridPos) -> Vec2 {
    Vec2::new(f32::from(cell.x) * 102.0, f32::from(cell.y) * 102.0)
}
// In tests/ui_layout.rs:
// assert_eq!(cell_origin(GridPos::new(8, 8)), Vec2::splat(816.0));
```

- [ ] Build all current unit glyphs, HP/awaiting indicators, selection/inspection rings and finished dimming. Sync by IDs. Read legal cells from domain queries and locked footprints from committed intents; retain overlap and extraction information. Decorations do not affect occupancy or intercept inputs.
- [ ] Adapt `play_battle_events` to token/board positions, flashes, damage and KO/environment feedback. Keep input locked until the queue drains. Remove Camera3d/glTF effects together; domain state is already final during playback. Do not reconstruct state from events or shake the entire UI.
- [ ] Add `RecentBattleLog` in `playback.rs`; `push(String)` inserts at front and truncates to six. Append formatted events once when dequeued; reset on restart/exit. Add this inline test where the log fields are accessible:

```rust
#[test]
fn recent_log_is_newest_first_and_bounded() {
    let mut log = RecentBattleLog::default();
    for n in 0..8 { log.push(format!("event {n}")); }
    assert_eq!(log.entries.len(), 6);
    assert_eq!(log.entries.front().unwrap(), "event 7");
    assert_eq!(log.entries.back().unwrap(), "event 2");
}
```

- [ ] Replace every old MissionAssets/15-scene readiness consumer, mesh picker, world-effect query and 3D-only component. Preserve useful loading/error states, not glTF dependencies. Remove old assets only after live references/includes are gone; update associated tests atomically.
- [ ] Wire restart/rebuild/opening → reconcile/sync → playback → input → HUD. Make Camera2d/root cleanup compatible with campaign transitions. Opening planning runs once after reset, with no stale effects.
- [ ] Run `cargo test --test presentation_app`, `cargo test --lib`, `cargo test --all-targets`, `cargo clippy --all-targets --all-features -- -D warnings`. Inspect native board, effects, restart and re-entry. Commit `feat: replace the battlefield and playback with flat native presentation`.

## Task 4 — Inspection, contextual commands, pilots and restart

**Files:** `interaction.rs`, new `battle_menu.rs`, `ui.rs`, `mod.rs`, `app.rs`; `tests/ui_interaction.rs`, existing interaction/presentation tests.
**Consumes:** canonical activation/phase, existing command routes, flat cells and playback lock.
**Produces:** one inspected ID, closed menu state, shared guarded dispatch, `next_ready_unit(&BattleState) -> Option<UnitId>`, `restart_allowed(BattlePhase, bool, bool, bool) -> bool`.

- [ ] Add failing tests for enemy/finished inspection, inspecting another player while active, re-focus, next after Wait, and no ready unit when finished. Rename selection to inspection where appropriate; keep `battle.active_unit()` authoritative.
- [ ] Implement next-ready in Vanguard/Gunner/Interceptor order. In Player phase prefer the active ID, otherwise the first living unfinished player. Outside Player return none. Disable no-op re-focus. Successful Wait uses existing Finish validation and selects the next eligible unit.

```rust
use scorpius::domain::model::{Reaction, UnitId};
use scorpius::mission::mission_one::mission_one;
use scorpius::presentation::interaction::next_ready_unit;

#[test]
fn next_ready_never_abandons_an_activation() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    assert_eq!(next_ready_unit(&battle), Some(UnitId(1)));
    battle.begin_activation(UnitId(1)).unwrap();
    assert_eq!(next_ready_unit(&battle), Some(UnitId(1)));
    battle.choose_reaction(UnitId(1), Reaction::Guard).unwrap();
    battle.finish_activation(UnitId(1)).unwrap();
    assert_eq!(next_ready_unit(&battle), Some(UnitId(2)));
}
```

- [ ] Implement Hidden/Root/Weapons/Stances menus and source styling. Back pops submenu; Escape first cancels targeting then closes root. Background deselection changes only UI. Assert movement/action flags, EN and active ID survive cancellation.
- [ ] Target routing precedes inspection for both tokens and cells. Test occupied enemy attacks, ally Aegis, invalid targets, scrim/menu/disabled clicks, and clicks as playback starts. One click emits at most one command. Keep invalid targeting active with feedback.
- [ ] Render eligible Aegis targets and actual skill state. Verify invalid Aegis does not consume it; Guard does not stack; Focus does not apply to Counter; Overdrive is disabled after Move. Extend adapters around existing domain tests, not duplicate combat formulas.
- [ ] Use the following restart predicate; `locked` includes playback or queued events. The adapter also checks Battle screen. Reuse restart/opening logic without changing credits/save/progression. Test R and pointer equivalence, Defeat Retry, denied Victory and pending advancement.

```rust
pub fn restart_allowed(
    phase: BattlePhase, assets_ready: bool, locked: bool, pending_transition: bool,
) -> bool {
    assets_ready && !locked && !pending_transition
        && matches!(phase, BattlePhase::Player | BattlePhase::Defeat)
}

#[test]
fn restart_cannot_bypass_victory_or_playback() {
    assert!(restart_allowed(BattlePhase::Player, true, false, false));
    assert!(restart_allowed(BattlePhase::Defeat, true, false, false));
    assert!(!restart_allowed(BattlePhase::Victory, true, false, false));
    assert!(!restart_allowed(BattlePhase::Player, true, true, false));
    assert!(!restart_allowed(BattlePhase::Player, true, false, true));
    assert!(!restart_allowed(BattlePhase::Player, false, false, false));
}
```

- [ ] Compare saved bytes/credits/mission/upgrades before and after restart. Assert a fresh battle, one opening and cleared menus/log/preview/effects with no award.
- [ ] Run `cargo test --test ui_interaction`, `cargo test --test presentation_app`, `cargo test --lib`. Exercise every menu edge/corner and existing M/1/2/3/P/C/G/E/F/Space/R keys plus Escape at fitted sizes. Commit `feat: wire reference battle commands to canonical gameplay`.

## Task 5 — Tactical HUD, objectives and results

**Files:** `ui.rs`, `battle_menu.rs`, `sync.rs`, shared campaign rendering as needed; `tests/ui_snapshots.rs`, `campaign_flow.rs`, `campaign_persistence.rs`, `presentation_app.rs`.
**Consumes:** typed HudSnapshot, AttackPreview, locked intents/objectives, credits, log and menus.
**Produces:** exact header/inspector/log/threat/preview/result layouts for all seven missions.

- [ ] Add failing snapshots for no inspection, ally/enemy/finished, active/inactive, no target, insufficient EN, pilot states and Resolve readiness. Build bars/pips from typed values, never by parsing display strings.
- [ ] Implement status/progress/credits/restart header and inspector/log. Populate locked-threat and target-aware preview cards from domain queries. Without a target, show weapon information but no fabricated target hit/crit values.
- [ ] Add fixtures for protected Gunner HP/rounds, courier exit/deadline, Gate Bulwark target, both overlapping Mission 5 batteries, and both bosses across thresholds. Assert existing intents stay locked while future planning changes. No archetype silently falls back to Rifleman.
- [ ] Render all primary/bonus variants in the same visual system. Bound long text/threat lists with accessible scroll/detail regions, not shrinking text, overlapping the board or dropping entries.
- [ ] Build victory/defeat scrim/card/ring/indicators and actual objective copy. Wait for playback to drain. Reuse existing completion persistence; failed saves keep the result open and state unchanged. Retry never pays; repeated Continue never duplicates payment.
- [ ] Run `cargo test --test ui_snapshots`, `cargo test --test campaign_flow`, `cargo test --test campaign_persistence`, `cargo test --all-targets`. Capture Mission 1 and later objective/result/error extensions. Commit `feat: complete tactical HUD and campaign-aware result presentation`.

## Task 6 — Deterministic native capture and comparison

**Files:** new `capture.rs`, `examples/ui_capture.rs`, `tools/compare_ui.py`; `Cargo.toml`, `mod.rs`, narrow `app.rs` hook; reference manifest; `docs/validation/hpa-480.md`.
**Consumes:** production renderer, fixture constructors and legal command routes from preceding tasks.
**Produces:** reproducible native pixels/comparisons, not a generic E2E/MCP dependency.

- [ ] Add an opt-in feature/example; ordinary tests do not run its window. All-features checks still compile the fixture code.

```toml
[features]
ui-capture = []

[[example]]
name = "ui_capture"
path = "examples/ui_capture.rs"
required-features = ["ui-capture"]
test = false
```

- [ ] Define feature-gated `CaptureOptions` with `scenario: String`, `output: PathBuf`, `width: u32`, `height: u32`, `seed: u64`, `time_ms: u64`. Accept `--scenario`, `--output`, `--size WIDTHxHEIGHT`, `--seed`, `--time-ms`. Invalid sizes/scenarios exit nonzero. Test parsing without a renderer.
- [ ] Use explicit seed-7 campaign/upgrades/action fixtures. Initialize an isolated temporary SaveFile before startup; never touch the platform save. Reach battle/results through authored constructors and legal commands; verify expected HP/phase/objectives. Record setup/actions; fixture shortcuts never appear in shipped UI.
- [ ] Capture the actual native Bevy window only after required assets/layout are ready. Set UI animation time deterministically; domain RNG is independent of frame count. Fail on missing assets, unmet fixture state or absent output. Include 0 ms/midpoint captures and motion evidence for pulses/effects.
- [ ] Implement named cases: title no-save/save/completed/error; Story lines 1/2/3; Briefing 1; battle idle/player/enemy/finished/weapons/stances/move/attack/Aegis/Focus/Overdrive/used/Resolve/playback; victory/defeat/save-error; Aftermath lines 1/2; hangar affordable/unaffordable/purchased/MAX/save-error; Ending; Missions 2–7 objective variants. Use names such as `battle-player`, `title-no-save`, `mission-6-objective`. Enumerate the other missions' dialogue lines and all edge-menu cases explicitly in the manifest.
- [ ] Align reference view data to legal native fixtures, recording data-only changes separately from originals; CSS/art remain unchanged. The original forced-result PNG is composition evidence only. Review same-style extensions before acceptance, with no masks hiding them.
- [ ] Implement `tools/compare_ui.py --reference PATH --actual PATH --output DIR`. Document Pillow as a development-only dependency. Require equal dimensions; write side-by-side, 50% overlay and absolute-difference PNGs and report differing-pixel count/max channel delta. Do not rescale, mask broadly or auto-approve by percentage. Test identical tiny images, one changed pixel and dimension mismatch.

```python
from PIL import Image, ImageChops
reference = Image.open(reference_path).convert("RGB")
actual = Image.open(actual_path).convert("RGB")
if reference.size != actual.size:
    raise ValueError("Reference and native capture dimensions differ")
Image.blend(reference, actual, 0.5).save(output_dir / "overlay.png")
ImageChops.difference(reference, actual).save(output_dir / "difference.png")
```

The script wraps this core with argument parsing/filesystem checks. The variables are parsed paths, not hardcoded external services.

- [ ] Compile/test the opt-in path and perform an end-to-end capture:

```bash
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo run --features ui-capture --example ui_capture -- \
  --scenario battle-player --size 1920x1080 --seed 7 --time-ms 0 \
  --output target/ui-capture/battle-player.png
python3 tools/compare_ui.py \
  --reference docs/references/hpa-480/aligned/battle-player.png \
  --actual target/ui-capture/battle-player.png \
  --output target/ui-capture/compare/battle-player
```

- [ ] Run the full primary-size matrix and representative board/menu/dialogue/result cases at secondary sizes and HiDPI. Commit fixtures, tooling, reference/extension decisions and evidence on this branch with `test: add HPA-480 native visual parity evidence tooling`.

## Task 7 — Integrated acceptance and same-PR closeout

**Files:** affected UI files for concrete defects; `README.md`, `CLAUDE.md`, `docs/validation/hpa-480.md`; relevant tests/reference manifest.
**Consumes:** implemented scope, captures and approved extension targets.
**Produces:** passing gates and reviewed native parity at a named implementation commit.

- [ ] Map every spec-section-9 criterion to a test, screenshot, motion capture or explicit manual result. An unchecked criterion keeps this PR draft; do not create a closeout ticket.
- [ ] Play all seven missions through normal input with purchases, bonus successes/failures, reloads between missions, restart, defeat Retry and completed Continue. Record before/after persisted values and a save-failure retry. Preserve domain regression tests despite renderer changes.
- [ ] Verify 1920×1080, 1280×720, 1600×900, 1600×1000 letterboxing and HiDPI pointer alignment. Exercise repeated transitions, every menu edge/corner, long text, multiple threats, Aegis, results and no hidden 3D loading dependency.
- [ ] Review original/aligned/extension/native/diff sets and animation recordings. Fix every unapproved discrepancy in type, geometry, art/crops, color, glyphs, pips, menus, shadows and scrims. Do not change reference CSS, loosen thresholds or mask screens to make a failure pass.
- [ ] Update README/CLAUDE for flat rendering, controls, restart/Skip and validation. Retire obsolete live glTF/mesh instructions and representation-only tests, but preserve historical reports. No runtime engine/version or save migration is introduced.
- [ ] Run gates at the implementation head, not only the planning commit:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

- [ ] Record final commit, outputs, device/DPI, asset versions, fixtures/actions, native images, comparisons, motion evidence and approvals. Identify rasterization-only exceptions narrowly; no blanket waiver.
- [ ] Commit `docs: record HPA-480 full native parity acceptance` only after evidence exists. Update this same PR from planning-only to implementation-complete; mark ready only after all scope and gates pass. Complete HPA-480 through this one PR.

## Coverage and present status

Tasks 1–2 cover references/assets/scaling and all six campaign screens. Tasks 3–5 cover Battle/Result, lifecycle/playback, missing interactions and whole-campaign variants. Task 6 provides deterministic evidence; Task 7 performs integrated acceptance without splitting delivery.

This draft contains planning documents and source metadata only. Writing this plan did not implement native UI changes, capture tooling or new tests. No game build/test pass or visual approval is claimed. Begin execution with reference intake on the existing branch, not another planning PR.
