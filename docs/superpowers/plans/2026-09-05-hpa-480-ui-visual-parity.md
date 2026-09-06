# HPA-480 — Native UI visual parity implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Match the supplied Scorpius UI across all eight screen families, implement its missing interactions, and preserve the working seven-mission game.

**Architecture:** Keep the existing domain/campaign state, commands and screen flow. Replace the presentation with a fixed-canvas native Bevy UI, a flat tactical board, typed snapshots and a few concrete styling/layout helpers. Adapt existing asset loading, event playback and tests rather than adding another runtime or simulation.

**Tech stack:** Rust 2024, Bevy 0.19, existing serde/serde_json, native UI, existing headless Rust tests; an opt-in native capture example and development-only image comparison utility.

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

**Continue on `hpa-480-ui-visual-parity`.** These tasks are internal checkpoints, not additional Linear tickets or PRs. Do not merge this documentation-only head. Conventional commits may accumulate on the same branch; the final PR is merged only after implementation and acceptance.

## 0. Baseline and file ownership

Baseline is `d981682840eb9147ba9eb7f7c56b2ceae88a3aed`. Read `CLAUDE.md`, the spec, and the current versions of the files below before editing. The new design explicitly supersedes the old Camera3d/glTF presentation constraint, not domain rules.

| Path | Responsibility after this change |
| --- | --- |
| `src/presentation/theme.rs` (new) | Concrete palette, typography, panel/button/pip/bar helpers; no registry or theme editor. |
| `src/presentation/layout.rs` (new) | Canvas fit/coordinate conversion, board positions and menu clamping. |
| `src/presentation/assets.rs` | Presentation image/font/glyph handles and asset readiness; retire the glTF-only catalog at cutover. |
| `src/presentation/screens/mod.rs`, `title.rs`, `dialogue.rs`, `briefing.rs`, `hangar.rs`, `ending.rs` (new) | Screen-specific layouts; dialogue shared by Story/Aftermath. |
| `src/presentation/campaign_ui.rs` | Existing actions, typed campaign snapshots, dialogue cursor, persistence routing and screen cleanup. |
| `src/presentation/battlefield.rs`, `sync.rs` | Flat grid/tokens/props, markers and synchronization from domain state. |
| `src/presentation/ui.rs` | HUD snapshots, header/inspector/log/threat/preview/result builders and updates. |
| `src/presentation/battle_menu.rs` (new) | Anchored command, weapon and stance menu rendering from typed state. |
| `src/presentation/interaction.rs` | Inspection/target routing, menu actions, ready navigation, restart and keyboard parity. |
| `src/presentation/playback.rs` | Ordered event effects, input lock, six-entry recent log. |
| `src/presentation/mod.rs`, `src/app.rs` | Resources, component exports, lifecycle and explicit system ordering. |
| `assets/ui/` (new images/decorations), existing `assets/vn/` | Supplied art and small static UI decorations; no runtime HTML. |
| `tests/ui_layout.rs`, `tests/ui_interaction.rs`, `tests/ui_snapshots.rs` (new) | Deterministic helper/interaction/view-data coverage. |
| Existing presentation/campaign tests and inline domain tests | Adapt renderer assertions; preserve rules, persistence and seven-mission coverage. |
| `src/presentation/capture.rs`, `examples/ui_capture.rs` (new, opt-in) | Fixed native screenshot scenarios using production rendering, separate from headless tests. |
| `tools/compare_ui.py` (new) | Same-sized image side-by-side/overlay/diff, no automatic broad-tolerance acceptance. |
| `docs/references/hpa-480/`, `docs/validation/hpa-480.md` | Source identity, original/state-aligned/extension references and final evidence ledger. |

No other engine crate, plugin suite, database or generic workflow layer is needed. A new helper named below is a proposed project API, not an assertion that Bevy provides it.

## Task 1 — Reference intake, assets and native layout proof

**Files:** reference directory; new `theme.rs`, `layout.rs`; additive changes to `assets.rs`, `mod.rs`; new `tests/ui_layout.rs`; `assets/ui/`. Keep old asset wiring working until Task 3 replaces all its consumers.

**Inputs:** source HTML and screenshot ZIP listed in `docs/references/hpa-480/reference-manifest.json`; existing four VN images.
**Outputs:** source/art inventory; theme helpers; `CanvasLayout::fit(Vec2) -> CanvasLayout`, `CanvasLayout::to_design(Vec2) -> Option<Vec2>`, `clamp_menu(Vec2, Vec2, Vec2) -> Vec2`; new presentation asset handles and readiness tests.

- [ ] Obtain the exact source files from the originating conversation, check SHA-256 against the manifest, and place the PNG references in durable repository/issue storage. The draft has metadata only: a direct Linear ZIP upload failed before this plan was written. Do not invent a replacement baseline. Remove that availability warning only after transfer succeeds.
- [ ] Read the actual JSON-decoded `__bundler/template`, not the loading thumbnail. Record all reference screens, hovered/selected/disabled states, measurements and resource IDs. Keep the original screenshots separate from new captures and note that the old animation pause instant is unspecified.
- [ ] Extract only the named PNG art resources and compare the four reused VN assets. The five new image destinations are `assets/ui/key_art.png`, `briefing.png`, `vanguard.png`, `gunner.png`, `interceptor.png`. Record their source hashes; no demo JavaScript becomes runtime code.

The supplied bundle can be decoded for inspection with this development-only Python procedure (the project does not need a new asset pipeline):

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

- [ ] Add failing layout tests. Define the helper contracts as above, then run `cargo test --test ui_layout` and observe failing assertions before the implementation.

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

- [ ] Implement fit with the minimum width/height scale and centered offset, return no design coordinate outside the fitted canvas, and clamp menu origin to `[0, bounds - menu_size]`. Add tests for 1920×1080, 1280×720, 1600×900, each edge, and zero/minimized window size. Convert physical to logical coordinates only at the window boundary.
- [ ] Implement concrete theme helpers and the new image/font readiness catalog. Match source families, weights, tracking and crops. Validate title typography plus one complex card in a real Bevy window now; include hover/disabled states and the pulse treatment. Resolve discrepancies before multiplying the components across screens.
- [ ] Run `cargo fmt --check`, `cargo test --test ui_layout`, `cargo test --test presentation_app`, and `cargo check --all-targets`. Record the proof screenshots and asset-load failure behavior. Commit on the existing branch with `feat: add HPA-480 visual foundation and reference assets`.

## Task 2 — Campaign screens and truthful navigation

**Files:** `campaign_ui.rs`, new `screens/` modules, `app.rs`, `mod.rs`; `tests/campaign_flow.rs`, `campaign_persistence.rs`, `ui_snapshots.rs`.
**Consumes:** Task 1 theme/layout/assets; existing `CampaignUiAction`, `CampaignRuntime`, `DialogueCursor`, `CompletionReceipt`.
**Produces:** all seven campaign-facing screen layouts (Result remains Task 5); `CampaignUiAction::SkipDialogue`; typed campaign view fields and unchanged persistence flows.

- [ ] Extend existing fixtures with failing tests for Skip from any pre-mission line, rapid duplicate advances at transition, missing/corrupt/completed saves, and aftermath reading the just-completed mission. Check cursor/screen destination and unchanged credits/next mission, not only visible strings.
- [ ] Pin the reuse rule with existing pure routing in `tests/campaign_flow.rs`: a Skip action queues `GameScreen::Briefing`, does not invoke completion, and leaves the serialized campaign state unchanged. Add the enum arm and keep the action valid only on the pre-mission screen. Pending screen changes block additional clicks.

The routing addition has this shape inside the existing action match; callers supply the current-screen/pending guard:

```rust
CampaignUiAction::SkipDialogue => {
    next_state.set(GameScreen::Briefing);
}
```

- [ ] Move layout construction only into `screens/`; retain campaign actions/persistence in the existing adapter. Rebuild Title, Story, Briefing, Aftermath, Hangar and Ending exactly as spec section 5. Reuse the same dialogue builder for Story/Aftermath; add Skip only to Story.
- [ ] Derive progress pips and screen badges from saved state. For Briefing, read the current definition and construct/read a mission fixture only at screen entry when board/enemy metadata is needed; do not build a battle every UI frame or consume gameplay RNG.
- [ ] Render the persisted completion receipt and existing purchase results. Keep `persist_purchase` as the mutation owner; unaffordable/MAX states are inert and cannot pass through to another control. Keep Next Drop → next pre-mission story, final aftermath → Ending, completed Continue → Ending.
- [ ] Add formatted snapshot tests for the four upgrade tracks. Pin Mobility as evasion, costs/caps from existing constants, and no visual change on failed save. Include success/error status treatment within the reference layout.
- [ ] Run `cargo test --test campaign_flow`, `cargo test --test campaign_persistence`, `cargo test --test campaign_model`, `cargo test --test ui_snapshots`, and `cargo check --all-targets`. Capture each campaign screen plus unavailable/purchased/MAX states before committing `feat: match campaign screens to the Scorpius reference`.

## Task 3 — Flat tactical rendering, asset cutover and playback

**Files:** `battlefield.rs`, `sync.rs`, `playback.rs`, `assets.rs`, `mod.rs`, `app.rs`; existing inline visual/asset tests, `tests/presentation_app.rs`.
**Consumes:** `BattleRuntime`, `GridPos`, unit IDs, existing event queue/input lock, Task 1 layout/assets.
**Produces:** one production flat board/token renderer and one active presentation asset gate; retained ordered playback with native-grid effects.

- [ ] Add failing tests for one cell per board position, one glyph per living unit, correct blocker/hazard/explosive/extraction markers, and no duplicate camera/root after re-entry/restart. Replace renderer-specific glTF fixture assumptions with equivalent full-roster visual-catalog assertions; keep combat assertions.
- [ ] Implement a flat board under the same logical canvas as the HUD. Use source cell/gap/token sizes for the 9×9 reference; read board dimensions from state. Export a project helper `cell_origin(GridPos) -> Vec2` in `layout.rs` for board-local positions. At the reference pitch it returns `(x * 102, y * 102)`.

```rust
use bevy::prelude::Vec2;
use scorpius::domain::board::GridPos;
use scorpius::presentation::layout::cell_origin;

#[test]
fn grid_geometry_matches_reference_pitch() {
    assert_eq!(cell_origin(GridPos::new(8, 8)), Vec2::splat(816.0));
}
```

- [ ] Build native cells, unit glyphs/HP bars/awaiting dots, selection/inspection rings and finished dimming. Map every existing archetype explicitly. Sync by IDs; decorators never change occupancy. Render legal move/attack cells from domain queries and committed telegraphs from locked intents. Preserve overlapping contributions and extraction markers.
- [ ] Adapt `play_battle_events` to native token/board positions, small flashes, floating damage and KO/environment feedback. Retain `EventPlayback::input_locked` until the ordered queue drains; remove `Camera3d` and glTF effect dependencies together. Domain state remains final immediately, even while effects are playing.
- [ ] Add `RecentBattleLog` in `playback.rs`, with `push(String)` inserting at the front and truncating to six rows. Format and append each dequeued event once. Never drive HP, phase or credits from this log. Reset it with other transient battle state.

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

- [ ] Cut over all consumers to the presentation catalog: replace old `MissionAssets`/15-scene readiness checks, mesh picking, world-space effects, sync queries and 3D-only components. Keep useful asset-status/error behavior, not the old glTF requirement. Delete old scene assets only after no live include/reference remains. Update old asset tests instead of leaving `include_str!` references to deleted files.
- [ ] Rewire explicit schedule order: restart/rebuild/opening → reconcile/sync → playback → input → HUD. Ensure new camera/root ownership is compatible with campaign cleanup. A reset runs authored opening planning once and does not retain old effects.
- [ ] Run `cargo test --test presentation_app`, `cargo test --lib`, `cargo test --all-targets`, and `cargo clippy --all-targets --all-features -- -D warnings`. Inspect actual board/playback and restart/re-entry captures. Commit `feat: replace the battlefield and playback with flat native presentation`.

## Task 4 — Inspection, contextual commands, pilots and restart

**Files:** `interaction.rs`, new `battle_menu.rs`, `ui.rs`, `mod.rs`, `app.rs`; `tests/ui_interaction.rs`, existing inline interaction and presentation tests.
**Consumes:** canonical activation/phase, existing routing/commands, Task 3 cells and playback lock.
**Produces:** one inspected unit ID; closed `BattleMenu` state; shared pointer/keyboard dispatch; `next_ready_unit(&BattleState) -> Option<UnitId>` and `restart_allowed(BattlePhase, bool, bool, bool) -> bool` helpers.

- [ ] Add failing tests covering enemy/finished inspection, inspecting another ready player while an activation exists, re-focusing active unit, automatic next after Wait, and no ready unit after all finish. Rename the existing presentation selection field/call sites to reflect inspection; active authority stays in `BattleState`.
- [ ] Implement next-ready selection in fixed squad order. During Player phase return the existing active ID first; otherwise return the first living unfinished player. Outside Player return no candidate. UI disables a no-op re-focus. Wait only succeeds through the existing Finish validation.

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

- [ ] Implement menu states `Hidden`, `Root`, `Weapons`, `Stances` and source rows/icons. Back pops a submenu; Escape clears tentative targeting before closing the root. Background deselection clears inspection/menu only. Add tests that moved/acted flags, EN and active ID survive cancellation.
- [ ] Route token/cell clicks through the same target path before any inspect logic. Add regression tests for attacking an occupied enemy cell, Aegis on an ally, invalid target, overlay input, disabled button clicks, and a button pressed in the same frame that playback starts. Check one command per click.
- [ ] Render legal Aegis targets and the actual Focus/Overdrive state. Test invalid Aegis preserves use count; Guard+Aegis does not stack; Focus is not consumed by Counter; Overdrive is unavailable after moving. Reuse existing domain tests and extend adapter assertions, not duplicate combat formulas.
- [ ] Allow Restart only under the following pure predicate; `locked` includes active playback OR queued events. The mutation adapter also checks current Battle screen and uses existing restart/opening logic. Reject victory and pending advancement. Test pointer and R produce the same result.

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

- [ ] Pin restart persistence: compare saved bytes/credits/next mission/upgrades before and after; confirm fresh battle, one opening, cleared inspection/menu/log/preview/effects, and no award. Add tests for Defeat Retry separately.
- [ ] Run `cargo test --test ui_interaction`, `cargo test --test presentation_app`, and `cargo test --lib`. Exercise all menu edges/corners at each fitted size and verify existing keys M/1/2/3/P/C/G/E/F/Space/R plus Escape. Commit `feat: wire reference battle commands to canonical gameplay`.

## Task 5 — Full tactical HUD, objectives and result states

**Files:** `ui.rs`, `battle_menu.rs`, `campaign_ui.rs` as needed for shared rendering, `sync.rs`; `tests/ui_snapshots.rs`, `campaign_flow.rs` and `presentation_app.rs`.
**Consumes:** extended `HudSnapshot`, domain `AttackPreview`/intents/objectives, campaign credits, recent log, menu state.
**Produces:** exact header/inspector/log/threat/preview/result layouts across Missions 1–7.

- [ ] Add snapshot assertions for no inspection, living/finished ally, enemy, active/inactive command availability, missing target, unavailable energy, pending skills and Resolve readiness. Use typed values directly; do not parse `selected_summary` strings to build bars/pips.
- [ ] Build the header counters/progress/credits/restart, inspector and six-row log. Construct the right column from locked-threat data and real target-aware previews. Distinguish no-target weapon information from hit/crit results; never display made-up target accuracy.
- [ ] Add whole-campaign fixtures: protected Gunner HP/rounds; courier exit/deadline; Gate Bulwark target; both Mission 5 overlapping batteries; Dreadnought and Regent threshold phases. Assert future planning can change the boss weapon while an existing locked intent remains unchanged. Test each archetype maps to a distinct intended glyph.
- [ ] Render each primary/bonus variant within the reference visual system. Make long text and larger threat lists accessible inside their bounded regions. Do not hide off-screen entries or stretch the board to fit text.
- [ ] Build victory/defeat overlay with full input scrim, reference dimensions and actual objective copy. Delay result interaction until playback drains. Reuse `complete_current_mission` through the existing Continue path; failed save leaves the overlay open and unchanged. Retry cannot pay, Continue cannot pay twice.
- [ ] Run `cargo test --test ui_snapshots`, `cargo test --test campaign_flow`, `cargo test --test campaign_persistence`, and `cargo test --all-targets`. Capture source-matched Mission 1 states plus later-objective/result/error extensions. Commit `feat: complete tactical HUD and campaign-aware result presentation`.

## Task 6 — Deterministic native capture and comparison matrix

**Files:** new `capture.rs`, `examples/ui_capture.rs`, `tools/compare_ui.py`; `Cargo.toml`, `mod.rs`, a narrow hook in `app.rs`; reference manifest and `docs/validation/hpa-480.md`.
**Consumes:** production rendering, existing fixture constructors and pure command routes, all preceding UI states.
**Produces:** reproducible native captures and comparison artifacts; no external E2E/MCP dependency.

- [ ] Add an opt-in `ui-capture` feature and example. Exclude running its window from ordinary tests; still compile it with the all-features gate. Keep fixture code out of the normal game path.

```toml
[features]
ui-capture = []

[[example]]
name = "ui_capture"
path = "examples/ui_capture.rs"
required-features = ["ui-capture"]
test = false
```

- [ ] Implement `CaptureOptions` in the feature-gated module: `scenario: String`, `output: PathBuf`, `width: u32`, `height: u32`, `seed: u64`, `time_ms: u64`. The example accepts `--scenario`, `--output`, `--size WIDTHxHEIGHT`, `--seed`, `--time-ms`; unknown scenarios/invalid dimensions fail with nonzero exit.
- [ ] Use seed 7 and explicit campaign/upgrades/actions per named fixture. Initialize an isolated temporary `SaveFile` before game startup; never touch the user's platform save. Reach battle/result states through authored constructors and legal domain commands. Verify fixture HP/phase/objective before capture. Record the exact setup/action sequence in the evidence ledger; do not expose fixture shortcuts in the shipped UI.
- [ ] Use native Bevy window screenshot capture after required assets and layout are ready; set UI animation clock to the requested time. Capture real pixels, then exit. Fail on missing assets, missing scenario completion or absent output. For animated states capture 0 ms and midpoint plus a short motion recording; do not change domain RNG based on frame count.
- [ ] Implement these scenario families: `title-no-save`, `title-save`, `title-complete`, `title-error`; `story-{1,2,3}`; `briefing-1`; `battle-idle`, `battle-player`, `battle-enemy`, `battle-finished`, `battle-weapons`, `battle-stances`, `battle-move`, `battle-attack`, `battle-aegis`, `battle-focus`, `battle-overdrive`, `battle-skill-used`, `battle-resolve`, `battle-playback`; `result-victory`, `result-defeat`, `result-save-error`; `aftermath-{1,2}`; `hangar-affordable`, `hangar-unaffordable`, `hangar-purchased`, `hangar-max`, `hangar-save-error`; `ending`; and `mission-{2,3,4,5,6,7}-objective`. Add the other missions' authored dialogue lines and all board-edge menu variants to the manifest as explicit cases, not a random traversal.
- [ ] State-align the HTML reference view to each legal native fixture with data-only binding changes, recorded separately from original captures. Preserve source CSS/art. The original forced-result screenshot is layout evidence only. Obtain review of same-style extensions for cases not supplied by the mockup; do not mask those regions out.
- [ ] Implement `tools/compare_ui.py --reference PATH --actual PATH --output DIR` with Pillow as a documented development-only dependency. Check identical dimensions, write side-by-side, 50% overlay and per-channel absolute difference PNGs, and report differing-pixel count/maximum channel delta. No rescaling, broad masks or percentage-based pass is built in. Use `Image.blend` and `ImageChops.difference`, not a new comparison framework.

```python
from PIL import Image, ImageChops
reference = Image.open(reference_path).convert("RGB")
actual = Image.open(actual_path).convert("RGB")
if reference.size != actual.size:
    raise ValueError("Reference and native capture dimensions differ")
Image.blend(reference, actual, 0.5).save(output_dir / "overlay.png")
ImageChops.difference(reference, actual).save(output_dir / "difference.png")
```

The script wraps this core in argument parsing and filesystem checks. Add tests using tiny generated images for identical input, one changed pixel, and size mismatch.

- [ ] Verify fixture/feature compilation and capture one case end to end:

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

- [ ] Repeat the fixture matrix at the primary size and representative battle/menu/dialogue/result states at the secondary sizes and HiDPI. Commit tooling, deterministic fixture descriptions and reference/extension decisions on this branch with `test: add HPA-480 native visual parity evidence tooling`.

## Task 7 — Integrated acceptance and same-PR closeout

**Files:** all affected UI files for concrete defects; `README.md`, `CLAUDE.md`, `docs/validation/hpa-480.md`; relevant tests/reference manifest.
**Consumes:** complete feature, all scenario captures and approved extension targets.
**Produces:** passing behavior/build gates and reviewed native visual parity at a named implementation commit.

- [ ] Walk the exact acceptance inventory in spec section 9. Pair every item with a test, native screenshot, motion capture or manual step/result. An unchecked item keeps this PR draft; do not create a closeout ticket.
- [ ] Execute the seven-mission campaign through normal input, with purchases, optional-objective successes/failures, save reload between missions, a restart, defeat Retry and final completed Continue. Preserve gameplay tests even when the native visual path changed. Record before/after save values around rewards/purchases and a failed-save retry.
- [ ] Verify pointer alignment and blocked input at 1920×1080, 1280×720, 1600×900, 1600×1000 and HiDPI. Check repeated screen transitions, all menu edges/corners, long text, multiple threats, Aegis targets, terminal overlays and no hidden 3D dependency.
- [ ] Review original/aligned/extension/native/diff sets. Fix every unapproved difference in typography, geometry, art/crops, colors, glyphs, pips, menus, shadows, scrims and animation. A score alone does not grant parity. Do not edit reference CSS, loosen thresholds or mask a screen to approve a mismatch.
- [ ] Update README/CLAUDE for flat rendering, controls, restart/Skip and validation commands. Remove obsolete live glTF/mesh-picking instructions and tests tied only to the retired representation; leave historical validation reports untouched. Record no runtime engine/version or save-model migration.
- [ ] Run the final gates at the implementation head, not only at the original planning commit:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

- [ ] Put final commit, command outputs, device/DPI, asset versions, scenario inputs, screenshots, comparison artifacts, animation evidence and reviewer approvals in `docs/validation/hpa-480.md`. Identify rasterization-only exceptions narrowly; no blanket waiver.
- [ ] Commit `docs: record HPA-480 full native parity acceptance` after evidence exists. Update this same PR description from planning-only to implementation-complete. Mark ready only after all tasks and the full acceptance contract pass; close HPA-480 through this one final PR.

## Plan coverage and current status

Tasks 1–2 cover source intake, styling, assets, scaling and every campaign screen. Tasks 3–5 cover the flat board, lifecycle/playback, all missing interactions, full HUD/results and later-mission variants. Task 6 makes capture/state alignment reproducible; Task 7 owns final integrated evidence without splitting delivery.

This draft contains planning documents and source metadata only. No native UI change, visual-capture tool or new tests have been implemented by writing this plan, and no game build/test pass or visual approval is claimed. The first execution step is reference intake on the existing branch, not another planning PR.
