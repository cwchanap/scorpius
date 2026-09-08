# HPA-480 — Native UI visual parity implementation plan

> **Delivery:** one Linear ticket, one branch, one PR. Continue on `hpa-480-ui-visual-parity` / PR #7. These tasks are internal checkpoints, not sub-issues or PRs. Do not create a fallback tag/partial-merge path unless the user changes the delivery contract.

**Goal:** Match the updated Scorpius UI, including the 2.5D/isometric Battle layout, while preserving the complete seven-mission game.

**Architecture:** Keep canonical Rust domain/campaign state and `GameScreen`. Put all screens under one fitted 1920×1080 Bevy UI canvas. Battle uses a deterministic 2.5D projection of the existing 9×9 `GridPos`, one stage-level analytical cell picker, upright UI tokens, minimal source-derived geometry masks, typed snapshot extensions, and the existing interaction/event/persistence paths. Retire the 3D/glTF/mesh-picking path atomically.

**Spec:** [2026-09-05-hpa-480-ui-visual-parity-design.md](../specs/2026-09-05-hpa-480-ui-visual-parity-design.md)

## Global constraints

- Rust 2024, Bevy 0.19, one crate, domain Bevy-free.
- One ticket/branch/PR; do not merge planning-only head.
- Result remains Battle overlay; Hangar remains `GameScreen::Upgrade`.
- One 1920×1080 fitted canvas for **all eight** screens.
- Current boards are intentionally fixed at 9×9 for this feature.
- 2.5D is presentation only; no `Camera3d`, physics, WebView, selectable renderer, or new simulation.
- Existing `InteractionMode` remains targeting. `MenuState` is **new** presentation chrome state introduced for the updated sidebar. Inspection is view-only; commands use `battle.active_unit()`.
- Extend existing `ObjectiveTrackSnapshot` / `ThreatSnapshot` / campaign helper call sites in place; do not add synonymous parallel models.
- Skip is PreMissionStory-only. Reuse `NextState<GameScreen>` as the pending transition signal.
- No save migration, new slots, checkpoints, undo, Settings, plugin registry, MCP, or generic E2E framework.
- Normal tests remain headless. Native capture is an example-only explicit command.
- Full native visual parity with the updated source is the final gate.

## 0. File ownership after HPA-480

| Path | Responsibility |
| --- | --- |
| `src/presentation/theme.rs` (new) | Sole production font/palette/UI-atlas/board-mask/unit-archetype style helpers; const atlas rect tables. |
| `src/presentation/layout.rs` (new) | Pure `CanvasLayout::fit`, diagnostic `to_design`, battle stage constants, isometric projection/inverse hit test. |
| `src/presentation/assets.rs` | PNG/font/UI-atlas/board-mask handles and `AssetLoadStatus`; no glTF catalog after cutover. |
| `src/presentation/screens/{mod,title,dialogue,briefing,hangar,ending}.rs` | Campaign layout construction only. |
| `src/presentation/campaign_ui.rs` | Existing campaign actions/cursor/persistence; typed campaign helpers; pending transition predicate. |
| `src/presentation/battlefield.rs`, `sync.rs` | 2.5D stage visuals, token/blocker/terrain reconciliation, no second selected-cell state. |
| `src/presentation/ui.rs` | Extend existing HUD/objective/threat snapshots; add `InspectorSnapshot`; Battle HUD/Result. |
| `src/presentation/battle_menu.rs` (new) | Fixed-left-sidebar Root/Weapons/Stances/targeting panel and new `MenuState`. |
| `src/presentation/interaction.rs` | Inspection/target routing, active-unit commands, Cancel, next-ready, restart, stage click/move/out observers, keyboard parity. |
| `src/presentation/playback.rs` | Ordered token/board effects, input lock, bounded log using `ui::format_event`. |
| `src/presentation/mod.rs`, `src/app.rs` | Resources, `UiScale`, picking settings, camera/root markers, resize/system order, 3D cutover. |
| `examples/ui_capture.rs`, `examples/ui_capture/fixtures.rs` | Opt-in deterministic native capture and typed fixture table only; `test = false`. |
| `tools/compare_ui.py` | Dev-only equal-size side-by-side/overlay/difference tool. |
| `docs/references/hpa-480/`, manifest, `docs/validation/hpa-480.md` | Byte/source provenance, required scenario IDs, source/extensions/native evidence. |

---

## Task 1 — Vendor the updated source contract, then prove canvas/2.5D math

**Files:** reference manifest/directory, `assets/ui/`, `assets/fonts/`, new `theme.rs`, `layout.rs`, additive `assets.rs`/`mod.rs`/`app.rs`, `tests/ui_layout.rs`.

### 1.1 Source and assets first

- [ ] Verify updated HTML size `12,278,676` and SHA-256 `04bbed2958cce4c3c2ddc665f5826fac32050f509db59f852a350acb299d6e19`.
- [ ] Preserve the old flat-source hash only as superseded provenance. Do not use old flat Battle/Result PNGs as final goldens.
- [ ] **Hash-reuse preflight:** inspect candidate repo/worktree images before copying. If a candidate exactly matches a required source SHA-256, adopt/rename it into the final committed path and remove the duplicate candidate. The baseline Git tree has no tracked `assets/generated/` directory, so do not assume local untracked candidates exist.
- [ ] Import exact `keyArt` and `briefArt` source bytes when no matching candidate exists. Reuse the existing tracked VN assets whose hashes match.
- [ ] Extract the seven exact Latin WOFF2 faces from the updated bundle. Convert once outside the normal build only as needed for Bevy-native loading, commit the resulting TTF bytes, and verify the manifest `native_ttf_sha256` values. **Do not replace source-derived faces with arbitrary upstream full TTF releases for parity.**
- [ ] Vendor the applicable SIL Open Font License text alongside the committed font assets. Do not add a converter/regeneration pipeline to the application.
- [ ] Re-export the updated source UI-vector catalog into `assets/ui/icons.png` and record final atlas hash.
- [ ] Export a **minimal geometry atlas** (`assets/ui/board.png`) containing approximately three source-exact grayscale/alpha primitives: diamond mask/base, raised blocker prism/top+face, token footprint/shadow. State colors use `ImageNode.color`; hazard/explosive/extraction symbols reuse the UI icon atlas/native overlays unless source geometry proves otherwise.
- [ ] Keep board/UI atlas rect tables as const Rust in `theme.rs`; the manifest stores hashes/provenance, not runtime geometry JSON.
- [ ] Commit the verified binary/source assets **before** theme/layout/component work. The commit must make the branch self-contained from git, not dependent on chat attachments or untracked local files.

### 1.2 Failing layout/projection tests

Add `tests/ui_layout.rs` first and confirm bad constants/assertions fail.

```rust
#[test]
fn all_screens_share_one_letterbox_transform() {
    let fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    assert!((fit.scale - 5.0 / 6.0).abs() < 0.00001);
    assert!((fit.offset - Vec2::new(0.0, 50.0)).length() < 0.001);
    assert!(fit.to_design(Vec2::new(800.0, 10.0)).is_none());
    assert!((fit.to_design(Vec2::new(800.0, 500.0)).unwrap() - Vec2::new(960.0, 540.0)).length() < 0.001);
}

#[test]
fn updated_battle_stage_and_iso_centers_are_pinned() {
    assert_eq!(
        battle_stage_rect(),
        Rect::from_corners(Vec2::new(456.0, 204.0), Vec2::new(1464.0, 968.0))
    );
    assert_eq!(iso_center(GridPos::new(0, 0)), Vec2::new(960.0, 394.0));
    assert_eq!(iso_center(GridPos::new(8, 0)), Vec2::new(1408.0, 618.0));
    assert_eq!(iso_center(GridPos::new(0, 8)), Vec2::new(512.0, 618.0));
    assert_eq!(iso_center(GridPos::new(8, 8)), Vec2::new(960.0, 842.0));
}

#[test]
fn inverse_iso_hit_test_returns_one_cell_or_none() {
    let stage = battle_stage_rect();
    let local = iso_center(GridPos::new(4, 7)) - stage.min;
    assert_eq!(grid_from_stage_point(local), Some(GridPos::new(4, 7)));
    assert_eq!(grid_from_stage_point(Vec2::new(4.0, 4.0)), None);
}
```

Also test all 81 centers, each diamond quadrant, shared edges, outside-stage points, and 9×9 rejection. `grid_from_stage_point` uses inverse projection plus diamond inclusion; it must never return two cells.

`CanvasLayout::to_design` is retained for pure capture/diagnostic tests only; board input must not call it.

### 1.3 Runtime canvas/picking/theme proof

- [ ] Add one percent-sized `ViewportRoot` that centers one fixed 1920×1080 `CanvasRoot` for every screen.
- [ ] Add `update_canvas_scale`, derive scale via `CanvasLayout::fit`, and write Bevy `UiScale`. Schedule it at startup/window resize in `PreUpdate` before `PickingSystems::Backend`.
- [ ] Insert `UiPickingSettings { require_markers: true, ..default() }`. Mark live UI cameras with `UiPickingCamera` and only intended interactive controls/stage/tokens with `Pickable`.
- [ ] Add resize coverage: after changing the logical window size and running the required update/layout frame, the stage hit maps to the same `GridPos` as before resize.
- [ ] Implement `theme.rs` as the only production `TextFont`/palette/icon/unit-archetype atlas mapping path. No `UnitGlyph` enum; exhaustive `UnitArchetype -> atlas rect/style` match with no fallback.
- [ ] Keep `AssetLoadStatus` as the one readiness/error gate.
- [ ] Render one typography-heavy Title state and one Battle inspector/token/diamond proof in a native window before multiplying components.

Run:

```bash
cargo fmt --check
cargo test --test ui_layout
cargo test --test presentation_app
cargo check --all-targets
cargo run
```

Record the proof commit/window in `docs/validation/hpa-480.md`.

---

## Task 2 — Typed campaign helpers, guarded navigation, campaign screens

**Files:** `campaign_ui.rs`, new `screens/`, `app.rs`, `mod.rs`; campaign tests and `tests/ui_snapshots.rs`.

### 2.1 Reuse current types/call sites

- [ ] Replace `briefing_copy` **in place** with `briefing_snapshot`; do not create a second parallel copy API. Snapshot fields: mission/title/enemy count/primary/optional/base reward/optional reward/credits. Board size is not repeated because HPA-480 asserts 9×9 once.
- [ ] Replace `upgrade_row_copy` in place with `UpgradeRowSnapshot` return data.
- [ ] Replace `ending_copy` consumers with typed ending data. Render `CompletionReceipt` fields directly; keep `DialogueSnapshot`.
- [ ] Derive briefing enemy count from one deterministic `definition.build` at screen entry. Do not hardcode HTML demo values or add board width/height fields to every mission definition.

### 2.2 Transition guard

- [ ] Add `CampaignUiAction::SkipDialogue`.
- [ ] Hoist one `screen_transition_pending(&NextState<GameScreen>)` predicate and call it at the top of `apply_campaign_action`; reuse the same predicate for victory Continue. Do not add a resource or duplicate test-only implementations.
- [ ] Skip succeeds only on `PreMissionStory`; Aftermath Skip returns status/no mutation.
- [ ] Test duplicate last-line Advance, Skip, Continue, and Proceed with a pending transition; state and reward bytes must remain unchanged.

### 2.3 Screens under the shared canvas

- [ ] Move layout construction only into `screens/`; keep campaign actions/persistence in `campaign_ui.rs`.
- [ ] Implement Title, Story, Briefing, Aftermath, Hangar, Ending under the same fitted canvas/`UiScale` path.
- [ ] Introduce a campaign camera marker/`UiPickingCamera`; stop `despawn_campaign_screen` from querying bare `Camera2d`.
- [ ] Explicitly mark clickable campaign controls with `Pickable` because UI picking requires markers.
- [ ] Preserve purchase/save failure atomicity and completed Continue/Ending routing.

Run:

```bash
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test campaign_model
cargo test --test ui_snapshots
cargo check --all-targets
```

---

## Task 3 — 2.5D board cutover, one cell picker, camera ownership, playback

**Files:** `battlefield.rs`, `sync.rs`, `playback.rs`, `assets.rs`, `interaction.rs`, `app.rs`, `mod.rs`, narrow `ui.rs`; presentation tests.

### 3.1 Camera/root failure first

- [ ] Add a failing Title → Battle → exit → Battle/restart test. Exactly one marked live screen camera/root may exist. Campaign teardown must not despawn the Battle camera during an overlapping transition frame.
- [ ] Add `CampaignCamera` / `BattleCamera2d` (or equivalent narrow markers) and make cleanup target those markers. Update the current campaign cleanup comment that claims bare `Camera2d` cleanup cannot touch Battle; that comment becomes false once Battle is 2D.

### 3.2 Replace 3D atomically

- [ ] Remove `Camera3d`, `MeshPickingPlugin`, 15-scene `MissionAssets`, `grid_to_world`, world-only reconciliation/effects, and boss camera shake only when their 2.5D replacements are wired.
- [ ] Keep `AssetLoadStatus`; replace its catalog with required PNG/font/UI-atlas/board-mask handles.
- [ ] Replace `scene_index` with exhaustive `UnitArchetype -> theme atlas rect/style`. No second enum and no Rifleman fallback.

### 3.3 2.5D rendering with Bevy depth

- [ ] Render the 1008×764 stage under the common canvas using the fixed projection constants.
- [ ] Spawn/update flat cell images/telegraphs/highlights, 26px raised blockers, extraction/hazard/explosive visuals, and 76×64 upright unit cards.
- [ ] Use `ImageNode.color` for state/tile tints instead of baked color variants.
- [ ] Keep interleaving stage items as a **flat sibling list** and assign Bevy `ZIndex`: flat layers below 10; blocker `10 + (x+y)*3`; token `11 + (x+y)*3`; explicit feedback above as required.
- [ ] Finished ally dimming, HP bars/numbers, awaiting dot, selection/inspection footprint/shadow follow the updated source.

### 3.4 One board cell picker + live hover

The updated isometric diamonds overlap in their rectangular UI bounds. Do **not** put independently clickable rectangular nodes on every cell.

- [ ] The Battle stage is one explicitly `Pickable` UI node. Board decorations are unmarked/non-pickable.
- [ ] `Pointer<Click>` uses the UI picking backend's normalized node-local hit position, converts it directly to stage pixels with `(hit.position.xy + 0.5) * Vec2(1008, 764)`, and calls `grid_from_stage_point`.
- [ ] `Pointer<Move>` performs the same conversion continuously and calls `update_hover_preview`; this replaces the old per-cell `Pointer<Over>` behavior so hover/attack preview cannot freeze after entering the single stage node.
- [ ] `Pointer<Out>` clears `hovered_cell`, preview, and preview-cell rendering.
- [ ] Do not call `CanvasLayout::to_design` in the production pointer path.
- [ ] Token cards are explicitly `Pickable` UI nodes. During targeting, token click resolves current `unit.position` and calls the same `route_cell_click`; during Inspect it inspects the unit.
- [ ] One click emits at most one routed command. Test shared diamond edges, token-over-cell, blocker-over-cell, target-mode precedence, resize/HiDPI alignment.
- [ ] Remove/fold `SelectedCell`; highlights read `InteractionState.hovered_cell`/mode only.

### 3.5 Playback/log

- [ ] Make existing `ui::format_event` `pub(crate)` and append once when playback dequeues an event into six-entry `RecentBattleLog`.
- [ ] Adapt move/attack/damage/KO/environment effects to stage/token design positions; do not infer domain state from events.
- [ ] Input stays locked until event queue drains; terminal overlay waits for relevant playback.

Headless gates:

```bash
cargo test --test presentation_app
cargo test --test ui_layout
cargo test --lib
cargo clippy --all-targets --all-features -- -D warnings
```

**Native visual gate before Task 4:**

```bash
cargo run
```

Play Mission 1 through to Victory using the cutover board and currently wired command path. Record commit + smoke evidence in `docs/validation/hpa-480.md`. Inspect ZIndex interleave, token/blocker occlusion, footprint layering, hover movement, targeting, playback, and camera lifecycle. Do not defer the first human view of the new battlefield to Task 5/6.

---

## Task 4 — Reuse HUD types, inspection/commands, new fixed-sidebar `MenuState`

**Files:** `ui.rs`, `interaction.rs`, new `battle_menu.rs`, `app.rs`, `mod.rs`; `tests/ui_interaction.rs`, `ui_snapshots.rs`.

### 4.1 Extend current snapshot types in place

- [ ] Keep `ThreatSnapshot`; change `cells: String -> Vec<GridPos>` and add IDs needed by rendering.
- [ ] Grow existing `ObjectiveTrackSnapshot` with `EliminateAll`, and add round/deadline/position/escape data to existing Protect/Intercept/Target cases.
- [ ] Add `OptionalProgressSnapshot` mirroring `OptionalObjective`.
- [ ] Add the genuinely new `InspectorSnapshot`; replace `selected_summary` string use.
- [ ] Keep `HudSnapshot::from_battle` as the one builder. Leaf renderers format text; no snapshot parser.

### 4.2 Interaction composition

- [ ] Rename selection state to `inspected_unit` where presentation-only; commands validate/return `battle.active_unit()`.
- [ ] Test enemy/finished inspection during an active player activation; active unit remains untouched.
- [ ] Implement stable next-ready; successful Wait starts/focuses next living unfinished unit.
- [ ] Add `CommandAction::Cancel`; pointer Cancel and Escape share it. Invalid Move/Attack/Aegis keeps targeting active.
- [ ] Preserve Aegis/Focus/Overdrive domain semantics and keyboard command parity.
- [ ] Restart/Retry uses one `restart_allowed` predicate with asset/playback/pending checks and no progression mutation.

### 4.3 Add and test `MenuState`

`MenuState` does not exist on the baseline; introduce it deliberately:

```rust
pub enum MenuState {
    Hidden,
    Root,
    Weapons,
    Stances,
}
```

- [ ] Build Root/Weapons/Stances as normal vertical blocks in the left 352px sidebar beneath inspector.
- [ ] During Move/Attack/Aegis targeting replace that area with the source targeting label/Cancel panel.
- [ ] Test `Root -> Weapons -> Back -> Root` and `Root -> Stances -> Back -> Root`.
- [ ] Test menu state when entering targeting, on invalid target, on Cancel, and after successful targeting.
- [ ] Test successful Wait hands off to next-ready and opens Root for the new active unit.
- [ ] Test inspecting enemy/finished unit while activation exists does not transfer command/menu authority.
- [ ] Delete board-relative anchoring/clamping implementation and tests. There are **no** 24 menu-edge visual cases in the updated design.

Headless gates:

```bash
cargo test --test ui_interaction
cargo test --test ui_snapshots
cargo test --test presentation_app
cargo test --lib
```

**Native interaction gate:**

```bash
cargo run
```

Play Mission 1 through to Victory using the final fixed-sidebar menus, target/cancel paths, hover preview, resolve/playback, and Result flow. Record the commit and observed issues/evidence in `docs/validation/hpa-480.md` before capture automation.

---

## Task 5 — Typed fixture-driven native capture, no duplicate golden tests

**Files:** `examples/ui_capture.rs`, `examples/ui_capture/fixtures.rs`, `tools/compare_ui.py`, `Cargo.toml`, manifest, validation doc.

### 5.1 Capture remains example-only

```toml
[features]
ui-capture = []

[[example]]
name = "ui_capture"
path = "examples/ui_capture.rs"
required-features = ["ui-capture"]
test = false
```

Do **not** add `src/presentation/capture.rs`. Ordinary `cargo test` remains headless; all-features may compile the example but must never launch a window during tests.

### 5.2 Typed Rust fixture table

Do **not** parse action/fact strings from JSON. The manifest contains required scenario IDs and byte provenance only.

Define closed fixture types beside the example:

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
    SetWindow(u32, u32),
    // closed setup-only variants when a legal authored path cannot express a renderer fixture
}
```

- [ ] Build `const CAPTURE_FIXTURES: &[CaptureFixture]` keyed by the manifest scenario IDs.
- [ ] Reuse real mission `ids::*`, `MissionId`, `GameScreen`, `CampaignUiAction`, `CommandAction`, `GridPos`, etc. A renamed ID should be a compile error.
- [ ] Use isolated temporary save files; never platform save.
- [ ] Reach Battle states through authored constructors and legal interaction/domain actions where possible. If a synthetic renderer-only setup is unavoidable, use `BattleState::new`/existing typed test fixture; never fake `MissionId`.
- [ ] Assert expected typed screen/phase/menu/mode/inspection/HP/objective facts before rendering.

### 5.3 Reduced evidence set

Do not visually golden-test pure helper permutations:

- no menu-edge matrix;
- no every-dialogue-line matrix;
- no PNG for every secondary size;
- no fabricated `long-objective-copy`, `roster-all-glyphs`, or `max-threat-list` when real missions/unit tests cover them.

Keep representative Title/Story/Briefing/Aftermath/Hangar/Ending states plus the rich Battle/HUD/objective/result matrix and actual later-mission archetype states.

Use 1920×1080 for primary parity and 1600×1000 for one representative letterbox visual. Test `CanvasLayout::fit` numerically at 1280×720, 1600×900, and 1600×1000. HiDPI is a stage-local pointer alignment test plus a sanity capture only if a renderer-specific discrepancy is found.

### 5.4 Comparison tool

`tools/compare_ui.py` requires equal-size images and emits side-by-side, 50% overlay, absolute-difference, differing-pixel count, and max channel delta. No rescale, broad masks, loose mismatch percentage, or automatic baseline acceptance.

Run:

```bash
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo run --features ui-capture --example ui_capture -- \
  --scenario battle-active-vanguard --size 1920x1080 --seed 7 --time-ms 0 \
  --output target/ui-capture/battle-active-vanguard.png
python3 tools/compare_ui.py \
  --reference docs/references/hpa-480/aligned/battle-active-vanguard.png \
  --actual target/ui-capture/battle-active-vanguard.png \
  --output target/ui-capture/compare/battle-active-vanguard
```

---

## Task 6 — Integrated acceptance and same-PR closeout

- [ ] Recapture the updated HTML at a recorded animation time. The old flat `battle*.png`/`result.png` files remain historical only.
- [ ] Review source/aligned/extension/native/diff sets for Title, representative campaign states, updated 2.5D Battle states, Result, later mission objective/archetype extensions, and motion.
- [ ] Play Missions 1–7 through normal commands including purchases, bonus success/failure, reload, restart, defeat Retry, final Ending, completed Continue, and one save failure/retry.
- [ ] Verify 9×9 projection, stage-edge hit testing, shared-diamond boundaries, Pointer<Move> hover, token depth vs blockers, token targeting precedence, sidebar menu/Cancel, one camera/root, resize alignment, no live 3D asset dependency, and no duplicate rewards.
- [ ] Update README/CLAUDE for 2.5D presentation, controls, Story Skip, Player-phase Restart, and validation.

Final gates at the implementation head:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

Record final commit, device/DPI, typed fixture table version, source/native captures, comparison output, motion evidence, and approved extension decisions in `docs/validation/hpa-480.md`. Only then mark PR #7 ready and HPA-480 Done.

## Present status

This branch is still planning-only. These revisions do not implement the 2.5D UI, tests, capture example, or binary asset intake, and no native build/parity claim is made.
