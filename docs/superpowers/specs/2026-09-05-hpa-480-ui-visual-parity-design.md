# HPA-480 — Complete native UI visual parity

**Date:** 2026-09-05 (America/Vancouver)  
**Revised:** 2026-09-07 for the supplied 2.5D gameplay layout and Bevy UI mechanism review  
**Issue:** [HPA-480](https://linear.app/cwchanap/issue/HPA-480)  
**Branch:** `hpa-480-ui-visual-parity`  
**Status:** Design for implementation; native UI parity is not yet implemented or verified.  
**Baseline:** `d981682840eb9147ba9eb7f7c56b2ceae88a3aed` on `main`.  
**Plan:** [Implementation plan](../plans/2026-09-05-hpa-480-ui-visual-parity.md)  
**Reference record:** [Reference manifest](../../references/hpa-480/reference-manifest.json)

## 1. Delivery decision

Deliver the complete visual overhaul as **one ticket, one branch, one PR**. The draft contains the design and plan; implementation, tests, reference intake, discrepancy fixes, and acceptance evidence continue on this same branch and PR. Do not merge a planning-only head, create implementation/closeout PRs, split plan phases into Linear sub-issues, or create a fallback tag for a partial merge unless the user explicitly changes this delivery contract.

Keep Rust 2024, Bevy 0.19, one application crate, the existing `GameScreen` flow, domain/campaign state, combat rules, progression, and save semantics. Result remains a Battle overlay and Hangar remains `GameScreen::Upgrade`.

The updated `Scorpius UI (offline)(1).html` supersedes the prior flat-battle reference wherever the two differ. The prior campaign-screen direction remains valid when unchanged, but the new bundle is the final source of truth for Battle/Result composition and shared chrome. Its simplified demo simulation remains non-authoritative; live Rust rules supply values and outcomes.

## 2. Keep/reuse

| Existing seam | Use in HPA-480 |
| --- | --- |
| `src/app.rs`: `GameScreen`, enter/exit systems, `enter_battle`, `teardown_battle_screen` | Keep screen flow; replace presentation ownership/cleanup. |
| `src/domain/*` | Keep authoritative movement, combat, intents, pilots, environment, RNG order, terminal rules. Domain remains Bevy-free. |
| `src/mission/*` | Keep authored 9×9 missions and IDs. Presentation reads them; it does not re-author rules. |
| `src/campaign/*` | Reuse New Game/Continue, `CompletionReceipt`, persistence, purchases, campaign completion. |
| `src/presentation/campaign_ui.rs` | Reuse `apply_campaign_action`, `DialogueCursor`, `dialogue_snapshot`, `campaign_destination`; replace layout builders and string copy helpers in place. |
| `src/presentation/interaction.rs` | Reuse `route_cell_click`, `execute_command`, restart/reset paths, keyboard/observer adapters. |
| `src/presentation/ui.rs` | Extend existing `HudSnapshot`, `ObjectiveTrackSnapshot`, and `ThreatSnapshot`; do not create synonymous parallel snapshot families. |
| `src/presentation/playback.rs` | Reuse ordered event consumption/input lock; add bounded log using the existing `ui::format_event`. |
| `src/presentation/assets.rs` | Keep `AssetLoadStatus` as the single presentation readiness/error gate. |
| Bevy UI picking | Use node-local hit coordinates from the UI picking backend; do not reconstruct window/DPI transforms for board input. |
| Bevy `UiScale` / `ZIndex` | Use the engine mechanisms for fitted fixed-pixel scale and sibling depth ordering instead of parallel transform/depth systems. |

Do not add a WebView, second UI framework, selectable old renderer, generic UI/plugin registry, save migration, save slots, checkpoints, undo, inventory, new missions, Settings page, MCP framework, or generic E2E framework.

## 3. One fitted canvas for all eight screens

All eight screen families live under one **1920×1080 design-pixel canvas**. `CanvasLayout::fit` remains pure/testable geometry:

```rust
pub const DESIGN_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);

pub struct CanvasLayout {
    pub scale: f32,
    pub offset: Vec2,
}

impl CanvasLayout {
    pub fn fit(window_logical: Vec2) -> Self;
    pub fn to_design(&self, window_point: Vec2) -> Option<Vec2>;
}
```

`to_design` is **capture/diagnostic only**. Production UI input does not use it. The Bevy UI picking backend already resolves window scale, letterbox placement, transforms, and DPI into node-local hit coordinates.

At runtime:

- an outer `ViewportRoot` fills the logical window with `percent(100)` and centers its child;
- the child `CanvasRoot` is exactly 1920×1080 fixed-pixel UI;
- `update_canvas_scale` writes `UiScale(fit.scale)`; the outer percent-sized root centers the scaled canvas, so production code does not hand-transform every node;
- `CanvasLayout.offset` remains useful for tests, capture crops, and diagnostics, not a second placement engine.

`update_canvas_scale` runs on startup and window-size changes, scheduled in `PreUpdate` before `PickingSystems::Backend`. Resize tests must run the app through the required layout/picking update and prove stage input remains aligned after resizing.

All interactive UI uses marker-required picking:

```rust
UiPickingSettings {
    require_markers: true,
    ..default()
}
```

The live screen camera carries `UiPickingCamera`; only intended interactive nodes carry `Pickable`. Decorations are inert by default instead of relying on every future decoration to remember `Pickable::IGNORE`.

One live screen owns one marked `Camera2d`. Campaign and Battle teardown target their camera/root markers rather than querying bare `Camera2d`, so one screen cannot delete another screen's camera during a transition.

## 4. 2.5D battle geometry

### 4.1 Presentation model

The new gameplay reference is **2.5D/isometric presentation over the existing 9×9 logical grid**, not a return to a 3D simulation. Retire the old `Camera3d`, `MeshPickingPlugin`, 15-scene `MissionAssets`, `grid_to_world`, world-space effects, and boss camera shake atomically once their 2.5D UI replacements exist. Do not keep dual renderers.

All seven authored missions are currently 9×9; HPA-480 treats that as a fixed presentation invariant and fails loudly for a non-9×9 active board rather than creating a generic board engine.

The reference battle keeps the existing 22px outer padding, 78px header, 14px gaps, and 352px sidebars. The middle column contains a centered **1008×764** battle stage. At 1920×1080 the stage is:

```text
left = 456
top = 204
width = 1008
height = 764
```

Stage-local constants from the source are:

```text
TILE_WIDTH = 112
TILE_HEIGHT = 56
BLOCK_HEIGHT = 26
ISO_ORIGIN_X = 504
ISO_ORIGIN_Y = 190
TOKEN_WIDTH = 76
TOKEN_HEIGHT = 64
```

The logical-to-stage projection is fixed:

```rust
pub const fn battle_stage_rect() -> Rect;
pub fn iso_center(pos: GridPos) -> Vec2;      // design-absolute center
pub fn tile_bounds(pos: GridPos) -> Rect;     // visual bounds only
pub fn depth_key(pos: GridPos) -> i16;        // x + y
pub fn grid_from_stage_point(local: Vec2) -> Option<GridPos>;
```

Equivalent source projection:

```text
stage_cx = 504 + (x - y) * 56
stage_cy = 190 + (x + y) * 28
depth = x + y
```

Therefore `GridPos(0,0)` is the top-center diamond, +X descends down-right, and +Y descends down-left. Do not describe `(0,0)` as visual top-left.

### 4.2 Rendering and depth

Use native Bevy UI under the fitted canvas. The board is not a sprite/world-space scene.

The board atlas stores **geometry masks, not state-colored duplicates**. Keep it to the smallest source-exact primitive set needed for parity, approximately:

1. one diamond mask/base geometry;
2. one grayscale raised-blocker prism/top+face geometry;
3. one token footprint/shadow geometry.

Alternating tiles, move/attack highlights, telegraphs, selection, inspection, and other diamond states reuse the same diamond rect with `ImageNode.color` driven by `theme.rs`. Hazard/explosive/extraction symbols use the UI icon atlas/native overlays unless the source proves a genuinely distinct board geometry is required. Palette iteration must not require re-exporting a PNG.

Atlas rectangle mappings are **const Rust data in `theme.rs`**, not runtime JSON. The manifest records source/binary provenance and the final atlas hash only.

Raised blockers use the source 26px vertical face. Upright unit tokens are 76×64, with a footprint/shadow beneath.

Depth uses Bevy `ZIndex`. All depth-sorted stage visuals that need to interleave are siblings in one flat stage child list; do not nest a token/blocker inside per-cell containers that defeat sibling `ZIndex` ordering. Equivalent source layers are:

```text
flat board layers < 10
blocker z = 10 + (x + y) * 3
token z   = 11 + (x + y) * 3
foreground feedback as explicitly authored above those values
```

Do **not** introduce a `UnitGlyph` enum that mirrors `UnitArchetype`. `theme.rs` maps `UnitArchetype -> atlas rect/style` with one exhaustive match and no fallback.

### 4.3 One board hit path

Per-cell Bevy UI picking is not the production hit model for the isometric board: adjacent 112×56 cell nodes have overlapping rectangular UI bounds.

The single battle-stage node is the board pick surface. Its `Pointer<Click>` and `Pointer<Move>` events use the picking backend's normalized node-local `HitData.position`:

```text
stage_local_px = (hit.position.xy + 0.5) * Vec2(1008, 764)
```

Then call `grid_from_stage_point(stage_local_px)`. That function uses the inverse isometric projection plus a diamond-inclusion check and returns one legal `GridPos` or `None`.

Input rules:

- `Pointer<Move>` recomputes `InteractionState.hovered_cell` and attack preview on every movement inside the stage;
- `Pointer<Out>` clears hover/preview;
- `Pointer<Click>` routes the computed cell once;
- upright token cards use ordinary UI observers and resolve current `unit.position` into the same `route_cell_click` path while targeting;
- in Inspect mode token observers inspect the token's `UnitId` without changing activation authority;
- noninteractive board visuals are unmarked/non-pickable under marker-required UI picking;
- targeting has precedence over inspection exactly once per click.

This is the sole cell picker. Do not also attach independent clickable rectangular `CellVisual` nodes, hand-roll window→design→stage input conversion, or restore mesh/sprite picking.

Fold the old `SelectedCell` presentation resource into `InteractionState.hovered_cell`/inspection; `sync.rs` must not retain a second selected-cell state after cutover.

## 5. Sidebar command layout

The updated source moved contextual commands into the fixed **left 352px sidebar** below the inspector. There is no board-anchored contextual menu and therefore no `clamp_menu` requirement.

`MenuState` is **new presentation state** introduced by HPA-480:

```rust
pub enum MenuState {
    Hidden,
    Root,
    Weapons,
    Stances,
}
```

It is justified by the updated drill-down sidebar; it is not existing code being preserved. Root contains Move, Attack, Stance, Skill, Wait. Weapons/Stances replace the root rows in the same sidebar region; Back returns to Root. During Move/Attack/Aegis targeting, the menu region becomes the source-style targeting/Cancel panel.

Explicit transition tests cover:

- `Root -> Weapons -> Back -> Root`;
- `Root -> Stances -> Back -> Root`;
- Root/submenu -> targeting hides menu;
- Cancel/success exits targeting to the correct menu state;
- successful Wait hands off to next-ready and sets the new active unit's Root state;
- inspecting an inactive/enemy unit never silently changes the active unit's menu authority.

Delete the old 24 `menu-{kind}-{edge}` acceptance cases. They belonged to the superseded flat contextual menu.

## 6. Interaction composition

```rust
pub struct InteractionState {
    pub inspected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode, // existing: Inspect | Move | Attack(WeaponId) | AegisTarget
    pub menu: MenuState,       // new: Hidden | Root | Weapons | Stances
    pub preview: Option<AttackPreview>,
}
```

`inspected_unit` is view-only. Every mutating command derives its subject from `battle.active_unit()`; replace `require_selected_active_unit` with an active-unit validator. Inspecting another unit never changes activation authority.

| Menu | Targeting mode | Inspected | Mutation subject |
| --- | --- | --- | --- |
| Hidden | Inspect | none / last | none |
| Root / Weapons / Stances | Inspect | active player | `battle.active_unit()` |
| Hidden | Move / Attack / AegisTarget | active player | `battle.active_unit()` |
| Hidden | Inspect | enemy / finished / inactive player | none; inspector only |

Clicking a ready player begins activation only when no activation exists. While another activation exists it is inspection-only. Next-ready uses Vanguard/Gunner/Interceptor order; while an activation exists it re-focuses that unit. After a **successful** Wait/`FinishUnit`, begin/focus the next living unfinished unit. All finished exposes Resolve.

Add `CommandAction::Cancel` so pointer Cancel and Escape share one guarded command path. Invalid Move/Attack/Aegis targets keep targeting active and do not spend resources. `route_cell_click` changes targeting state only after domain success. Background deselection is UI-only; it never undoes movement/action/stance.

Aegis, Focus, and Overdrive remain the existing domain mechanics. No UI-local consumption or formulas.

Restart remains a narrow flow extension: allowed only in idle Player phase and idle Defeat/Retry, rejected during loading/playback/planning/resolution/Victory/pending transition. It rebuilds the current authored mission with current upgrades and a fresh seed, without rewards/progression mutation.

## 7. Typed presentation data: extend, do not fork

The current HUD already owns `ObjectiveTrackSnapshot` and `ThreatSnapshot`. Extend those types in place rather than creating parallel `PrimaryProgressSnapshot`/replacement threat families.

Add the genuinely new `InspectorSnapshot`. Grow existing `ObjectiveTrackSnapshot` to cover EliminateAll, Protect, Intercept, and Target with the numeric fields required by bars/pips/cards. Add `OptionalProgressSnapshot` beside it, mirroring the existing closed `OptionalObjective` variants. Keep existing `ThreatSnapshot`; change `cells: String` to `Vec<GridPos>` and add attacker/weapon/intended-occupant IDs as needed. Do not ship a second threat struct.

`HudSnapshot::from_battle` remains the single battle snapshot builder. Leaf renderers format strings. The battle menu receives typed weapon/pilot rows; no combat formula is duplicated.

`RecentBattleLog` stores six formatted entries newest-first. `playback.rs` calls the existing `ui::format_event` once when an event is dequeued; do not add a second event formatter or queue.

For campaign UI, replace string helpers **in place** with typed return values/call sites:

- `briefing_copy` -> `briefing_snapshot` with mission/title/enemy count/objectives/rewards/credits;
- `upgrade_row_copy` -> `upgrade_row_snapshot`;
- `ending_copy` -> typed ending fields;
- Aftermath renders `CompletionReceipt` fields directly;
- `DialogueSnapshot` stays unchanged.

The 9×9 board size is a presentation invariant, not repeated `BriefingSnapshot` data. Derive briefing `enemy_count` from one deterministic `definition.build` at screen entry; do not hardcode prototype flavor data.

## 8. Campaign navigation

Add `CampaignUiAction::SkipDialogue`, legal only in `GameScreen::PreMissionStory`. Aftermath has no Skip.

Hoist one `screen_transition_pending(&NextState<GameScreen>)` predicate and reuse it in `apply_campaign_action` plus victory Continue. Do not add a pending-transition resource or a third ad-hoc check. A queued transition makes a duplicate click a no-op/status result without state mutation.

## 9. Typography and asset provenance

`theme.rs` is the sole production `TextFont`/palette/icon helper. Delete the two local `text_font` helpers during cutover. Chakra Petch and IBM Plex Mono weights are exact source-derived assets; fallback fonts are an asset error.

The updated bundle has source SHA-256 `04bbed2958cce4c3c2ddc665f5826fac32050f509db59f852a350acb299d6e19` and size `12,278,676` bytes. The five major art PNG bytes and seven required Latin WOFF2 font bytes match the prior bundle by SHA-256 even though resource UUIDs changed.

### Fonts

For pixel provenance, **do not replace the bundled subset faces with arbitrary upstream release TTFs**. The source actually rendered the bundled WOFF2 faces; a different complete release can carry different hinting/tables/rasterization even when family/weight names match.

Implementation performs any WOFF2→TTF conversion once outside the normal build, commits the resulting native TTF bytes, and verifies the manifest's native SHA-256. There is no converter script/policy in the application and no regeneration step in normal build/test. The committed TTFs are the Bevy source of truth; source WOFF2 hashes remain provenance.

Vendor the applicable SIL Open Font License text alongside the committed font assets.

### Art

Before copying image bytes, hash candidate repository/worktree assets. Reuse/rename an existing candidate only when its SHA-256 exactly matches the required source asset; never commit two paths containing the same required bytes. The Git baseline does not contain a tracked `assets/generated/` directory, so local untracked candidates are a Task 1 preflight, not a planning assumption. Import exact source `keyArt`/`briefArt` when no matching candidate exists.

The **first implementation commit must make the PR buildable from git** with the required reference/native assets; do not leave a dependency on the originating chat or an untracked local file.

The updated source's vector catalog must be re-exported from this bundle, not reused by old UUID/path identity. Ordinary UI icons live in the UI atlas. The board atlas contains only minimal geometry masks; its rect table lives in `theme.rs`. Record final atlas hashes after export.

## 10. Screen requirements

| Screen | Requirement |
| --- | --- |
| Title | Key art, wordmark/emblems, New Game/Continue, save pips/errors. |
| Story | Source dialogue composition, portrait, pips, Next, pre-mission Skip. |
| Briefing | Mission metadata/objectives/rewards/Deploy from typed snapshot. |
| Battle | Updated 2.5D isometric stage, upright tokens, raised blockers, fixed left command area, inspector/log, right threat/preview column, header/Resolve. |
| Result | Updated source Battle overlay; real terminal/result persistence. |
| Aftermath | Source dialogue layout + persisted receipt; no Skip. |
| Hangar | Three mech columns/four tracks/typed purchase states. |
| Ending | Completion treatment and typed upgrade summary. |

Later missions extend the same visual language for Flanker, Bulwark, Controller, Dreadnought, Regent, extraction markers, protect/intercept/target objectives, overlapping threats, and boss threshold states. No unknown-archetype fallback.

## 11. Capture and acceptance strategy

Capture is an **example-only**, opt-in native tool in `examples/ui_capture.rs` (`test = false`). Do not add a `src/presentation/capture.rs` library module that pulls window/capture concerns into the normal test graph. `tools/compare_ui.py` remains dev-only.

Capture fixtures are **typed Rust**, not a JSON action DSL. Put the fixture types/table beside the example, e.g. `examples/ui_capture/fixtures.rs`:

```rust
pub struct CaptureFixture {
    pub id: &'static str,
    pub profile: CaptureProfile,
    pub actions: &'static [CaptureAction],
    pub expect: &'static [ExpectedFact],
}

pub enum CaptureAction {
    Campaign(CampaignUiAction),
    Inspect(UnitId),
    Command(CommandAction),
    ClickCell(GridPos),
    HoverCell(GridPos),
    // closed setup actions only where a legal authored path cannot express the fixture
}
```

Use real `MissionId`, `UnitId`, `WeaponId`, `GameScreen`, `CommandAction`, and other closed Rust types. A renamed/removed ID must fail compilation instead of becoming a runtime string error inside a windowed capture run.

The JSON manifest owns byte provenance and the **required scenario ID list**, not action execution semantics. Synthetic states use `BattleState::new`/existing test fixture only when authored content cannot produce a renderer contract; never invent a fake `MissionId`.

Do not create visual goldens that merely re-test pure helpers:

- no 24 menu-edge screenshots;
- representative Story/Aftermath states rather than every dialogue line;
- one representative non-16:9 letterbox visual plus pure `CanvasLayout::fit` tests for declared sizes;
- HiDPI is primarily an input-alignment test;
- no fabricated long-objective/all-glyph/max-threat goldens when real missions plus unit tests cover those contracts.

Keep the richer Battle/HUD/objective/result matrix because those states materially change composition.

Primary parity is 1920×1080. Also test `CanvasLayout::fit` for 1280×720, 1600×900, and 1600×1000; use 1600×1000 as the representative letterbox capture.

Final evidence uses source/aligned/extension/native namespaces, side-by-side, overlay, absolute difference, and motion review. The old flat `battle*.png`/`result.png` captures are superseded and cannot be final Battle goldens.

## 12. Verification staging and final gates

The 2.5D cutover cannot wait until final capture for its first visual inspection. At the end of the board/playback cutover and again after command/sidebar wiring, run the native game, play Mission 1 through the exercised interaction path, and record the commit plus smoke evidence in `docs/validation/hpa-480.md`. Specifically inspect Z-order interleaving, blocker/token occlusion, footprints, hover movement, targeting, playback, and camera lifecycle.

At the final implementation head run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

Normal test execution remains headless. Windowed capture/manual game runs are invoked explicitly and are not required just to run `cargo test`.

The PR stays draft and HPA-480 stays incomplete until all eight screen families work with live data, 2.5D Battle/Result match the updated reference, all seven missions regress successfully, and native parity evidence is recorded in `docs/validation/hpa-480.md`.
