# HPA-480 — Native UI visual parity implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Match the supplied Scorpius UI across all eight screen families, implement its missing interactions, and preserve the working seven-mission game.

**Architecture:** Keep the existing domain/campaign state, commands, persistence, and `GameScreen` flow. Replace presentation with one 1920 × 1080 fitted Bevy UI canvas, a fixed 9 × 9 UI-board coordinate system, typed view snapshots, and one explicit interaction composition (`MenuState` + `InteractionMode` + view-only inspected ID). Retire the 3D/glTF/mesh-picking path atomically rather than keeping parallel renderers.

**Tech stack:** Rust 2024, Bevy 0.19, existing serde/serde_json and headless Rust tests; exact source-derived TTF assets, one static icon atlas, an opt-in native capture example, and a development-only image comparison utility.

**Spec:** [2026-09-05-hpa-480-ui-visual-parity-design.md](../specs/2026-09-05-hpa-480-ui-visual-parity-design.md)

## Global constraints

- One ticket, one branch, one PR; implement and accept on `hpa-480-ui-visual-parity` / PR #7.
- Do not merge the planning-only head or create implementation/closeout PRs or sub-issues.
- Rust 2024, Bevy 0.19, one application crate, committed Cargo.lock.
- Native Bevy presentation only; no WebView, second UI framework, physics engine, networking, generic ability/UI framework, MCP framework, or generic E2E framework.
- `src/domain/` remains Bevy-free; `BattleRuntime` and `CampaignRuntime` remain authoritative.
- Result remains an overlay inside Battle; Hangar remains `GameScreen::Upgrade`; Skip is pre-mission only.
- One 1920 × 1080 logical canvas; uniform fit/letterbox. Current authored battle boards are deliberately fixed at 9 × 9 for this feature.
- Preserve combat balance, authored missions, committed intents, pilot restrictions, RNG order, exactly-once rewards, purchases, and save semantics.
- No save migration/backward compatibility, new save slots, checkpoints, undo, new missions, inventory, or Settings screen.
- Existing normal tests remain headless; windowed capture is opt-in.
- Full native visual parity is the final gate; unit tests alone cannot satisfy it.

## 0. File ownership and contracts

Baseline: `d981682840eb9147ba9eb7f7c56b2ceae88a3aed`. Read `CLAUDE.md`, the spec, and current files before editing.

| Path | Responsibility after HPA-480 |
| --- | --- |
| `src/presentation/theme.rs` (new) | Only production font/color/panel/button/pip/bar/icon-atlas helpers. |
| `src/presentation/layout.rs` (new) | Canvas fit, `board_rect`, `cell_rect`, pointer conversion, menu clamp. |
| `src/presentation/assets.rs` | PNG/font/icon handles and `AssetLoadStatus`; no glTF catalog after cutover. |
| `src/presentation/screens/{mod,title,dialogue,briefing,hangar,ending}.rs` (new) | Campaign layouts only; Story/Aftermath share dialogue. |
| `src/presentation/campaign_ui.rs` | Campaign actions, transition guard, typed campaign snapshots, cursor, persistence, cleanup. |
| `src/presentation/battlefield.rs`, `sync.rs` | Bevy UI board/cells/tokens/props/telegraphs and ID-based reconciliation. |
| `src/presentation/ui.rs` | Typed `HudSnapshot`, inspector/objective/threat/result view data, `format_event`, HUD rendering. |
| `src/presentation/battle_menu.rs` (new) | Root/Weapons/Stances menu rendering only. |
| `src/presentation/interaction.rs` | `InteractionState`, click/command routing, next-ready, Cancel, restart, keyboard parity. |
| `src/presentation/playback.rs` | Ordered event effects/input lock and six-entry `RecentBattleLog`, reusing `ui::format_event`. |
| `src/presentation/mod.rs`, `src/app.rs` | Resources/components, one Camera2d per live screen, system order, 3D cutover. |
| `assets/ui/`, `assets/fonts/`, existing `assets/vn/` | Exact source-derived art/font/icon assets. |
| `tests/ui_layout.rs`, `ui_interaction.rs`, `ui_snapshots.rs` (new) | Deterministic helper/interaction/view-data coverage. |
| Existing campaign/presentation/domain tests | Preserve behavior; replace representation-only 3D assertions. |
| `src/presentation/capture.rs`, `examples/ui_capture.rs` (new, opt-in) | Named native capture fixtures using production rendering. |
| `tools/compare_ui.py` (new) | Equal-size side-by-side/overlay/absolute-difference output. |
| `docs/references/hpa-480/`, `docs/validation/hpa-480.md` | Provenance, scenario matrix, references/extensions/native evidence. |

New helper names below are project contracts, not claims that Bevy exposes the same functions.

---

## Task 1 — Verify source bytes, vendor presentation assets, then prove layout/typography

**Files:** `docs/references/hpa-480/`, `docs/references/hpa-480/reference-manifest.json`, `assets/ui/`, `assets/fonts/`; then new `theme.rs`, `layout.rs`; additive `assets.rs`, `mod.rs`; new `tests/ui_layout.rs`.

**Consumes:** current conversation HTML/ZIP whose SHA-256 values are already pinned in the manifest.
**Produces:** durable exact source references, five art PNGs, seven native font files, icon atlas, and:

```rust
pub struct CanvasLayout {
    pub scale: f32,
    pub offset: Vec2,
}

impl CanvasLayout {
    pub fn fit(window_logical: Vec2) -> Self;
    pub fn to_design(&self, window_point: Vec2) -> Option<Vec2>;
}

pub const fn board_rect() -> Rect;
pub fn cell_rect(pos: GridPos) -> Rect;
pub fn clamp_menu(anchor: Vec2, menu_size: Vec2, bounds: Rect) -> Vec2;
```

- [ ] **Verify the exact source attachments before touching UI code.** Recompute SHA-256 for the HTML and ZIP and compare to `f3b74553...d3ef` and `3e471e24...75ef`. Copy the ten source PNGs into durable project/reference storage, keep their names/hashes unchanged, and link the actual storage location in the PR/manifest.

- [ ] **Extract and verify the five named PNG art resources and four reused VN files.** Destinations and hashes are already in the manifest. No prototype JavaScript becomes runtime code.

- [ ] **Extract the seven pinned Latin WOFF2 resources from the HTML manifest, convert them deterministically to the TTF destinations recorded in `reference-manifest.json`, and verify the recorded TTF SHA-256 values.** Do not fetch unpinned font versions. The needed set is Chakra Petch 400/500/600/700 and IBM Plex Mono 400/500/600. Commit the font/license assets as part of this implementation PR; do not add language subsets the current English game never renders.

- [ ] **Export the source inline vectors once.** The HTML contains 55 SVG occurrences / 51 exact variants / 40 unique geometries with catalog hashes already recorded. Render the closed semantic list into `assets/ui/icons.png`, define the atlas rect mapping in `theme.rs`, record the finished atlas SHA-256 in the manifest, and verify all 40 semantic slots resolve. No generic icon or emoji fallback.

- [ ] **Commit the verified reference/font/art/icon bytes before theme/layout implementation.** Suggested commit: `chore: vendor HPA-480 reference presentation assets`.

- [ ] **Write failing layout tests.** Run `cargo test --test ui_layout` and confirm assertion failures after the symbols exist but before correct constants are filled in.

```rust
use bevy::prelude::{Rect, Vec2};
use scorpius::domain::board::GridPos;
use scorpius::presentation::layout::{CanvasLayout, board_rect, cell_rect, clamp_menu};

#[test]
fn fit_and_pointer_conversion_share_one_letterbox_transform() {
    let fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    assert!((fit.scale - 5.0 / 6.0).abs() < 0.00001);
    assert!((fit.offset - Vec2::new(0.0, 50.0)).length() < 0.001);
    assert!(fit.to_design(Vec2::new(800.0, 10.0)).is_none());
    assert!((fit.to_design(Vec2::new(800.0, 500.0)).unwrap() - Vec2::new(960.0, 540.0)).length() < 0.001);
}

#[test]
fn source_board_and_cell_coordinates_are_pinned() {
    assert_eq!(
        board_rect(),
        Rect::from_corners(Vec2::new(504.0, 130.0), Vec2::new(1416.0, 1042.0))
    );
    assert_eq!(
        cell_rect(GridPos::new(0, 0)),
        Rect::from_corners(Vec2::new(504.0, 130.0), Vec2::new(600.0, 226.0))
    );
    assert_eq!(
        cell_rect(GridPos::new(8, 8)),
        Rect::from_corners(Vec2::new(1320.0, 946.0), Vec2::new(1416.0, 1042.0))
    );
}

#[test]
fn menu_clamps_inside_the_board_rect() {
    let bounds = Rect::from_corners(Vec2::ZERO, Vec2::splat(912.0));
    assert_eq!(
        clamp_menu(Vec2::new(900.0, 900.0), Vec2::new(236.0, 320.0), bounds),
        Vec2::new(676.0, 592.0)
    );
}
```

- [ ] **Implement `CanvasLayout`, exact board/cell geometry, and menu clamp.** `GridPos(0,0)` maps to visual top-left; `y=8` is the bottom row. A zero/minimized window yields no actionable canvas. Reject any active board not 9 × 9.

- [ ] **Implement `theme.rs` and asset readiness.** Centralize every production `TextFont` choice there and remove default-font assumptions from new components. Before replicating screen components, launch one typography-heavy Title state and one complex Battle card and visually verify font loading, tracking, weights, atlas icons, hover/disabled styling, and the fitted canvas.

- [ ] **Run foundation gates:**

```bash
cargo fmt --check
cargo test --test ui_layout
cargo test --test presentation_app
cargo check --all-targets
```

Commit `feat: add HPA-480 visual foundation`.

---

## Task 2 — Typed campaign snapshots and guarded campaign screens

**Files:** `campaign_ui.rs`, new `screens/`, `app.rs`, `mod.rs`; `tests/campaign_flow.rs`, `campaign_persistence.rs`, `ui_snapshots.rs`.

**Consumes:** Task 1 theme/layout/assets; existing `CampaignUiAction`, `CampaignRuntime`, `DialogueCursor`, `CompletionReceipt`, `persist_purchase`.
**Produces:** `CampaignUiAction::SkipDialogue`, `screen_transition_pending`, typed campaign snapshots, six campaign layouts.

- [ ] **Write failing transition tests** for Skip from every pre-mission line, Skip miswired from Aftermath, duplicate last-line advance, duplicate Continue/Proceed while `NextState` is already pending, missing/corrupt/completed saves, and aftermath reading the just-completed mission. Assert cursor, destination, receipt, and serialized state.

```rust
fn screen_transition_pending(next: &NextState<GameScreen>) -> bool {
    !matches!(next, NextState::Unchanged)
}

#[test]
fn a_queued_transition_rejects_a_second_campaign_action_without_mutation() {
    let before = state.clone();
    let mut next = NextState::Pending(GameScreen::Briefing);
    apply_campaign_action(
        GameScreen::PreMissionStory,
        CampaignUiAction::SkipDialogue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(runtime.0.state.as_ref().unwrap(), &before);
    assert_eq!(next, NextState::Pending(GameScreen::Briefing));
}
```

- [ ] **Extend `apply_campaign_action` with explicit current-screen input and top-level pending guard.** `SkipDialogue` is legal only for `PreMissionStory`; otherwise set `CampaignStatus` and do nothing. Keep Skip off Aftermath. Reuse `NextState` itself—do not create a duplicate pending-transition resource.

- [ ] **Replace campaign display blobs with typed snapshots.** Introduce exactly the spec types `BriefingSnapshot`, `UpgradeRowSnapshot`, `HangarSnapshot`; render `CompletionReceipt` fields directly; build Ending from typed credits/upgrade levels. Delete `briefing_copy`, `aftermath_reward_copy`, `ending_copy`, and `upgrade_row_copy` when their layouts no longer consume them.

```rust
#[test]
fn upgrade_snapshot_exposes_numbers_instead_of_formatted_copy() {
    let row = upgrade_row_snapshot(&state, PlayerMech::Vanguard, UpgradeTrack::Mobility);
    assert_eq!(row.level, 0);
    assert_eq!(row.current_bonus, 0);
    assert_eq!(row.next_bonus, Some(5));
    assert_eq!(row.cost, Some(200));
    assert_eq!(row.affordable, state.credits >= 200);
}
```

- [ ] **Move only layout construction into `screens/`.** Implement Title, Story, Briefing, Aftermath, Hangar, Ending exactly from spec; share dialogue layout. Do not move interaction/persistence into screen modules.

- [ ] **Render truthful campaign data.** Briefing uses current definition plus one deterministic mission build to read board/enemy metadata at screen entry, not every frame. Title progress derives from save state. Hangar derives costs/caps/effects from existing progression data. Mobility remains evasion.

- [ ] **Keep failures atomic.** Failed purchase/save leaves levels/credits/screen unchanged; inert controls block pass-through. Completed Continue routes to Ending; final Aftermath routes to Ending; Next Drop routes to the next pre-mission story.

- [ ] **Run campaign gates:**

```bash
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test campaign_model
cargo test --test ui_snapshots
cargo check --all-targets
```

Capture all campaign scenarios already enumerated in the manifest as implementation evidence when the capture tool becomes available. Commit `feat: match campaign screens to the Scorpius reference`.

---

## Task 3 — Cut the battlefield to one Bevy UI coordinate system and adapt playback

**Files:** `battlefield.rs`, `sync.rs`, `playback.rs`, `assets.rs`, `ui.rs` (`format_event` visibility only here), `mod.rs`, `app.rs`; `tests/presentation_app.rs`, `ui_layout.rs`; relevant inline asset tests.

**Consumes:** Task 1 canvas/board/icon contracts, domain IDs/state, existing event queue/input lock.
**Produces:** one UI-board renderer, exhaustive `UnitGlyph`, six-entry recent log, no live glTF/mesh-picking path.

- [ ] **First failing cutover test: one Camera2d/root only.** Exercise Title → Battle → exit → Battle/restart and assert there is never a duplicate Camera2d or battle root. This must fail before renderer replacement if cleanup is wrong.

- [ ] **Add failing board tests** for 81 exact cell rects, blockers/hazards/explosives/extraction, token placement, and a loud failure for non-9 × 9. Assert visual `GridPos(0,0)` is top-left and Mission 1 player `y=8` is bottom.

- [ ] **Replace mesh cells with UI nodes under the fitted canvas/board.** Each cell carries `CellVisual(GridPos)` and pointer observers; each token carries `UnitVisual(UnitId)`. Token clicks resolve current `unit.position` and call the same cell route. Do not create sprite/world coordinates.

- [ ] **Replace `scene_index` with an exhaustive glyph match.** No fallback:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitGlyph {
    Vanguard, Gunner, Interceptor, Rifleman, Striker, Artillery,
    Flanker, Bulwark, Controller, Dreadnought, Regent,
}

pub const fn glyph_for(archetype: UnitArchetype) -> UnitGlyph {
    match archetype {
        UnitArchetype::Vanguard => UnitGlyph::Vanguard,
        UnitArchetype::Gunner => UnitGlyph::Gunner,
        UnitArchetype::Interceptor => UnitGlyph::Interceptor,
        UnitArchetype::Rifleman => UnitGlyph::Rifleman,
        UnitArchetype::Striker => UnitGlyph::Striker,
        UnitArchetype::Artillery => UnitGlyph::Artillery,
        UnitArchetype::Flanker => UnitGlyph::Flanker,
        UnitArchetype::Bulwark => UnitGlyph::Bulwark,
        UnitArchetype::Controller => UnitGlyph::Controller,
        UnitArchetype::Dreadnought => UnitGlyph::Dreadnought,
        UnitArchetype::Regent => UnitGlyph::Regent,
    }
}
```

- [ ] **Adapt sync/telegraphs/highlights to board-local UI geometry** using `cell_rect`; preserve overlap and extraction. Decorative nodes are `Pickable::IGNORE` where required so the owning cell/token remains the hit target.

- [ ] **Adapt `play_battle_events` to UI-local effects while retaining queue ordering/input lock.** Make the existing `ui::format_event` `pub(crate)`. `RecentBattleLog::push(format_event(&event, &battle))` runs once as an event is dequeued; newest first, maximum six. Do not create another formatter.

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

- [ ] **Remove the 3D path atomically after replacements exist:** `MissionAssets` scene catalog, `scene_index`, `grid_to_world`, `Camera3d`, `DirectionalLight`, `MeshPickingPlugin`/settings, 3D materials/meshes, world-to-viewport damage text, boss camera shake, glTF-only readiness/tests. Keep `AssetLoadStatus` with new image/font/icon handles.

- [ ] **Wire lifecycle order:** restart/rebuild/opening → UI reconcile/sync → playback → input → HUD. Opening planning runs once after restart; stale effects/log/menu do not survive.

- [ ] **Run cutover gates:**

```bash
cargo test --test presentation_app
cargo test --test ui_layout
cargo test --lib
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Inspect native board/re-entry/restart before proceeding. Commit `feat: replace the battlefield with the flat native grid`.

---

## Task 4 — Make inspection view-only and unify command/menu/target state

**Files:** `interaction.rs`, new `battle_menu.rs`, `ui.rs`, `mod.rs`, `app.rs`; `tests/ui_interaction.rs`, existing interaction tests.

**Consumes:** canonical `battle.active_unit()`, Task 3 cells/tokens, `EventPlayback` lock.
**Produces:** explicit `InteractionState` composition, `CommandAction::Cancel`, `next_ready_unit`, `restart_allowed`.

- [ ] **Write failing tests for the composition contract:** enemy inspection, finished/inactive ally inspection while Vanguard remains active, re-focus active, target clicks versus inspection, invalid Aegis, Cancel, next after Wait, and all finished.

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuState { #[default] Hidden, Root, Weapons, Stances }

pub struct InteractionState {
    pub inspected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode,
    pub menu: MenuState,
    pub preview: Option<AttackPreview>,
}
```

The invariant tested in every mutation path is: **inspection is view-only; commands use `battle.active_unit()`**.

- [ ] **Replace `require_selected_active_unit` with `require_active_unit(battle)` and rename selection call sites to inspection.** Move/Attack/Aegis target routing reads the domain active ID. Inspecting another unit must never abort or transfer activation.

- [ ] **Make `route_cell_click` transactional with respect to targeting state.** Move/Attack already reset mode only after successful domain calls; make Aegis identical. An invalid Aegis target leaves `AegisTarget`, preview/highlight, and skill availability intact.

```rust
InteractionMode::AegisTarget => {
    let ally = battle.occupant_at(clicked).ok_or(BattleError::NoUnitSelected)?;
    battle.use_aegis(ally)?;
    interaction.mode = InteractionMode::Inspect;
    interaction.menu = MenuState::Root;
    interaction.hovered_cell = Some(clicked);
    interaction.preview = None;
    Ok(Vec::new())
}
```

- [ ] **Implement Root/Weapons/Stances chrome using the spec table.** Inspecting enemy/finished/inactive player hides commands; re-focusing active opens Root. Back only changes menu chrome.

- [ ] **Add `CommandAction::Cancel` and route both pointer Cancel and Escape through `run_command`.** Target Cancel returns `InteractionMode::Inspect`, clears preview, re-focuses active, opens Root. At Root, Cancel closes the menu. It never changes `moved`, `acted`, EN, reaction, or committed events.

- [ ] **Implement next-ready in Vanguard/Gunner/Interceptor order.** If an activation exists, return that ID. Otherwise return the first living unfinished player. On **successful** FinishUnit/Wait, call next-ready and begin/focus the next unit; do not add a keyboard-only implementation.

```rust
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

- [ ] **Targeting precedes inspection for token and cell clicks.** Test occupied enemy Attack, Aegis ally, background, disabled menu, overlay, and playback-lock clicks. One pointer event emits at most one command.

- [ ] **Render Aegis/Focus/Overdrive availability from domain state** and retain existing domain tests for non-stacking/consumption/timing.

- [ ] **Use existing `NextState` pending detection in restart/continue paths.** No new pending resource. Restart predicate:

```rust
pub fn restart_allowed(
    phase: BattlePhase,
    assets_ready: bool,
    locked: bool,
    pending_transition: bool,
) -> bool {
    assets_ready && !locked && !pending_transition
        && matches!(phase, BattlePhase::Player | BattlePhase::Defeat)
}
```

Test idle Player, idle Defeat, loading, playback/queue lock, planning/resolution, Victory, and pending transition. Compare save bytes/credits/mission/upgrades before/after restart.

- [ ] **Guard victory Continue before persistence.** If `NextState` is already pending, return without another `complete_current_mission` call. Keep `AlreadyAdvanced` as persistence backstop, not routine duplicate-click behavior.

- [ ] **Run interaction gates:**

```bash
cargo test --test ui_interaction
cargo test --test presentation_app
cargo test --lib
```

Exercise pointer and M/1/2/3/P/C/G/E/F/Space/R/Escape parity. Commit `feat: wire reference commands to canonical activation state`.

---

## Task 5 — Replace formatted HUD blobs with typed cards and cover every mission variant

**Files:** `ui.rs`, `battle_menu.rs`, `sync.rs`, `campaign_ui.rs` only for shared typed data if required; `tests/ui_snapshots.rs`, `campaign_flow.rs`, `campaign_persistence.rs`, `presentation_app.rs`.

**Consumes:** typed domain/campaign state, Task 4 inspection/menu, locked intents, `RecentBattleLog`.
**Produces:** typed `HudSnapshot`/cards and complete Result presentation for Missions 1–7.

- [ ] **Write failing typed-snapshot tests** before rendering cards. Remove expectations that depend on parsing `selected_summary`, `round_phase`, `primary`, `optional`, or stringified threat cells.

```rust
#[test]
fn inspector_snapshot_exposes_bar_and_status_fields() {
    let battle = mission_one(7);
    let hud = HudSnapshot::from_battle(&battle, Some(UnitId(1)), mission_definition(MissionId::One).unwrap());
    let inspector = hud.inspector.unwrap();
    assert_eq!(inspector.id, UnitId(1));
    assert_eq!(inspector.hp, inspector.max_hp);
    assert!(!inspector.moved);
    assert!(!inspector.acted);
    assert!(!inspector.finished);
}

#[test]
fn threat_cells_remain_grid_positions() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::One).unwrap());
    assert!(hud.threats.iter().all(|threat| !threat.cells.is_empty()));
    assert!(hud.threats.iter().flat_map(|t| &t.cells).all(|cell| cell.x < 9 && cell.y < 9));
}
```

- [ ] **Implement the spec's typed fields:** `InspectorSnapshot`, `ThreatSnapshot { cells: Vec<GridPos> }`, `PrimaryProgressSnapshot`, `OptionalProgressSnapshot`, typed weapon rows, pilot status, round/phase, resolve/restart/result availability. Leaf renderers may format labels; upstream snapshots never encode cards into multiline strings.

- [ ] **Render header, inspector, HP/EN bars/pips, recent log, menus, target preview, and locked threats** from typed data. Without an actual target show weapon data only; do not invent hit/crit values.

- [ ] **Cover all objective variants in tests and captures:**
  - Mission 2 protect full/low HP and round cap.
  - Mission 3 courier far/near/deadline/exit.
  - Mission 4 Bulwark target and Chain Reaction.
  - Mission 5 overlapping committed batteries and `VictoryByRound { current, cap }`.
  - Mission 6 Dreadnought above/below threshold.
  - Mission 7 Regent above/below threshold and Final Push progress.
  - All eleven glyphs, long objective copy, maximum threat list.

- [ ] **Build Result overlay from true terminal/result/objective data.** Wait for playback to drain. Continue uses existing persistence and Task 4 pending guard; failed save keeps result open; Retry never pays.

- [ ] **Bound overflow with source-style scroll/detail regions.** Do not shrink typography, overlap the board, or silently drop threats.

- [ ] **Run HUD gates:**

```bash
cargo test --test ui_snapshots
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets
```

Inspect every manifest battle/whole-campaign scenario manually before capture automation. Commit `feat: complete typed tactical HUD and results`.

---

## Task 6 — Deterministic capture, full scenario matrix, and same-PR acceptance

**Files:** new `capture.rs`, `examples/ui_capture.rs`, `tools/compare_ui.py`; `Cargo.toml`, `mod.rs`, narrow `app.rs` hook; `reference-manifest.json`; `docs/validation/hpa-480.md`; any UI file with a concrete discrepancy.

**Consumes:** production renderer plus the **already-enumerated** manifest scenario matrix.
**Produces:** reproducible native captures/comparisons and final implementation evidence.

- [ ] **Add an opt-in capture feature/example; normal tests stay headless.**

```toml
[features]
ui-capture = []

[[example]]
name = "ui_capture"
path = "examples/ui_capture.rs"
required-features = ["ui-capture"]
test = false
```

- [ ] **Define a closed parser/type from the manifest matrix rather than arbitrary `String` scenarios.** It may use grouped variants such as `Story { mission, line }` and `MenuEdge { menu, anchor }`, but validation accepts exactly the matrix values and rejects everything else. Test every manifest ID parses and one unknown ID fails.

- [ ] **Use explicit fixture inputs:** isolated temporary SaveFile, seed 7, named credits/upgrades, legal action routes, fixed animation `time_ms`. Never touch the platform save. Verify expected phase/HP/objective before capture.

- [ ] **Capture only after `AssetLoadStatus::Ready` and one live Camera2d/root.** Missing assets, unexpected fixture state, duplicate cameras, or absent output is an error.

- [ ] **Implement the comparison utility** with equal-dimension enforcement, side-by-side, 50% overlay, absolute difference, differing-pixel count and max channel delta. No rescale, broad mask, or percentage-based auto-approval.

```python
from PIL import Image, ImageChops
reference = Image.open(reference_path).convert("RGB")
actual = Image.open(actual_path).convert("RGB")
if reference.size != actual.size:
    raise ValueError("Reference and native capture dimensions differ")
Image.blend(reference, actual, 0.5).save(output_dir / "overlay.png")
ImageChops.difference(reference, actual).save(output_dir / "difference.png")
```

- [ ] **Run every closed primary-size scenario from `reference-manifest.json`.** There is no Task 6 discovery/enumeration step. Record original/state-aligned/extension/native/comparison paths separately. Review same-style extension targets before accepting them.

- [ ] **Run secondary geometry/input checks** at 1280 × 720, 1600 × 900, 1600 × 1000 and HiDPI. Exercise all 24 menu-edge cases and confirm pointer targets remain aligned with the visual board.

- [ ] **Play the full campaign manually through normal input** with bonus success/failure, purchases, reloads between missions, restart, defeat Retry, save failure, duplicate transition clicks, completed Continue, and final Ending. Preserve all domain regression tests.

- [ ] **Update README/CLAUDE** for the flat UI renderer, controls, restart/Skip, asset/capture commands. Preserve historical design/validation docs; remove only obsolete live glTF/mesh instructions.

- [ ] **Run final gates at the implementation head:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

- [ ] **Record final evidence in `docs/validation/hpa-480.md`:** implementation commit, asset hashes, scenario IDs, seed/state/actions, viewports/DPI, animation time, native images, comparisons, motion evidence, behavioral results, and any narrowly approved rasterization-only exception.

- [ ] **Keep PR #7 draft until all evidence is accepted.** Fix unapproved discrepancies on this same branch; do not loosen the reference, create a closeout ticket, or split the implementation. Final documentation commit: `docs: record HPA-480 full native parity acceptance`.

## Plan self-review

- **Spec coverage:** Tasks 1–2 cover exact reference/assets/fonts/icons/scaling and six campaign screens. Task 3 owns the coordinate/camera/3D cutover and shared event log. Task 4 owns the explicit interaction composition and guards. Task 5 owns typed battle view data and all mission variants. Task 6 owns the already-closed capture matrix and integrated acceptance.
- **Placeholder scan:** No implementation requirement is deferred with “TBD”, “TODO”, “similar to”, or an unspecified acceptance list. Exact asset/source hashes and scenario dimensions are in the manifest.
- **Type consistency:** `InteractionState.inspected_unit` is view-only; all mutating commands use `battle.active_unit()`. `MenuState` is chrome and `InteractionMode` is targeting. `ThreatSnapshot.cells` stays `Vec<GridPos>`. Campaign cards use typed snapshots/`CompletionReceipt`. `RecentBattleLog` reuses `ui::format_event`.

This branch is still planning-only at this point. No native UI code, builds, tests, captures, or visual acceptance are claimed by this document update.
