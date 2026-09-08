# Task 1 foundation report

Date: 2026-09-07
Base: `c546c87` (`fix: preserve HPA-480 icon atlas styles`)
Scope: Task 1.2/1.3 foundation only

## Result

The additive foundation is implemented and headless verification is green. The
existing 3D/glTF battlefield remains in place for the Task 3 atomic cutover.
The corrected UI atlas and all other committed assets were left untouched.

## Implementation

- Added `src/presentation/layout.rs` with the fixed 1920x1080
  `ViewportRoot`/`CanvasRoot` hierarchy, `CanvasLayout::fit`, diagnostic
  `to_design`, authored 1008x764 stage constants, isometric projection and
  inverse diamond hit test, plus `update_canvas_scale`.
- Added `src/presentation/theme.rs` with source palette values, exact bundled
  font paths and weight selection, board/UI atlas rectangles, image-node
  helpers, and one exhaustive `UnitArchetype -> UnitArchetypeStyle` match.
  No `UnitGlyph` type or fallback mapping was added.
- Extended `src/presentation/assets.rs` with typed UI art/atlas/font handles.
  `monitor_mission_assets` still owns the single `AssetLoadStatus` gate and
  now reports UI asset failures or waits for all required UI dependencies.
- Wired the shared canvas, `UiScale`, and marker-required
  `UiPickingSettings` in `src/app.rs`, with scale refresh before
  `PickingSystems::Backend`.
- Added `CampaignCamera`/`UiPickingCamera` ownership markers and parented
  campaign/HUD roots under the shared canvas. The existing 3D camera is also
  marked for current HUD picking; the 3D path is intentionally retained.
- Added `tests/ui_layout.rs` coverage for the prescribed projection cases,
  all 81 cell centers/quadrants, shared edges and outside rejection, canvas
  hierarchy, and resize scale/hit alignment.

## Verification evidence

Commands run after the final atlas-cell correction:

```text
rtk cargo fmt --check
(no output; exit 0)

rtk cargo test --test ui_layout
cargo test: 8 passed (1 suite, 0.02s)

rtk cargo check --all-targets
cargo build (1 crates compiled)
Finished `dev` profile [optimized + debuginfo]

rtk cargo test --all-targets
cargo test: 257 passed (7 suites, 0.40s)

rtk cargo clippy --all-targets --all-features -- -D warnings
cargo clippy: No issues found
```

The presentation integration suite also passed after the final correction:
`rtk cargo test --test presentation_app` — 23 passed.

TDD evidence: the inherited layout tests covered the initial projection
contract; this completion added the runtime canvas/resize tests and theme
contract assertions. The final run is green for all eight layout tests. A
deliberate bad-constant red run was not retained because the worktree already
contained the partial implementation when this scoped task resumed.

Asset intake is represented by the preceding committed asset checkpoint. The
corrected atlas remains SHA-256
`ffba26d5e0450ef72e50d3d72a622cb55b215665ef14fc3cf731cb1d7459cb7f`; this
foundation diff contains no asset paths.

## Changed files

```text
src/app.rs
src/presentation/assets.rs
src/presentation/battlefield.rs
src/presentation/campaign_ui.rs
src/presentation/layout.rs
src/presentation/mod.rs
src/presentation/theme.rs
src/presentation/ui.rs
tests/ui_layout.rs
```

The unknown untracked file `0` was excluded. No asset file was edited.

## Open gates and concerns

- Native `cargo run` proof could not be observed because the Mac window is
  locked. The typography-heavy Title state and Battle inspector/token/diamond
  state therefore remain unobserved and must be validated after unlock. This
  is a pending gate, not a passing visual claim.
- The resize test runs the real `PreUpdate` scale system with the required
  picking ordering and verifies the same stage-local cell after resizing, but
  remains headless and does not produce a native pointer screenshot.
- At the initial foundation checkpoint, the two local `text_font` helpers in
  `ui.rs` and `campaign_ui.rs` still used Bevy defaults. The review-fix round
  below supersedes that interim state and routes their live call sites through
  the exact-face theme helper.

## Review-fix round

Date: 2026-09-07. Review base: `48424b3`, with corrected asset checkpoint
`c546c87`.

The review findings are resolved within the foundation scope:

- Live campaign, battle HUD, and playback text now receive the one `UiAssets`
  font handle set and route through `theme::text_font`/`theme::chakra_petch`.
  The theme module is the only production location that constructs a
  `TextFont`; `UiAssets::from_world` remains the only bundled font loading
  model, so the exact face and weight cannot drift through a second
  `AssetServer::load` path.
- `teardown_battle_screen` filters the existing `BattleCamera` marker instead
  of every `Camera`. The focused `battle_teardown_preserves_unowned_cameras`
  test removes the battle camera and preserves an unrelated camera.
- `resize_recomputes_scale_before_picking_and_preserves_stage_cell_hits` now
  runs the real headless Bevy UI layout and picking backend. It lays out a
  `Node` stage under the production `CanvasRoot`, drives a synthetic pointer
  through `HoverMap`, resizes the test window from 1920x1080 to 1600x1000,
  refreshes `UiScale` before picking, and proves the same stage remains hit
  after the resized layout is available. No renderer/window startup or Task 3
  3D cutover was added.
- Added the pure fit-size pins for 1280x720 and 1600x900 alongside the existing
  1600x1000 letterbox case.

TDD/verification evidence: the focused resize and ownership tests were added
as regression seams and iterated against the actual Bevy resource and
visibility requirements. The initial minimal fixture exposed Bevy's missing
asset/input resource panic; adding only the concrete `UiPlugin` prerequisites
resolved it. The first pointer pass also exposed the missing visibility
component in the renderer-free fixture; an explicit visible stage marker then
made the backend hit observable. Final command output is recorded in
`docs/validation/hpa-480.md` and below:

```text
rtk cargo fmt --check
PASS (exit 0)

rtk cargo test --test ui_layout
PASS — 8 passed, 0 failed

rtk cargo test --test presentation_app
PASS — 23 passed, 0 failed

rtk cargo test --all-targets
PASS — 258 passed, 0 failed (7 suites)

rtk cargo clippy --all-targets --all-features -- -D warnings
PASS — no issues found (exit 0)
```

The native gate remains explicitly pending: the Mac was locked, so no
`cargo run`, title screenshot, battle screenshot, or live pointer observation
was available. The validation document records this as an unobserved visual
proof requirement for the coordinator after unlock. The unknown untracked
file `0` and all asset files remain excluded from this fix.

## Review-fix round 2

Date: 2026-09-08. Scope is limited to the resize fixture, validation note, and
this report appendix; no production or asset files changed.

The backend-alignment finding is resolved in
`resize_recomputes_scale_before_picking_and_preserves_stage_cell_hits`:

- The test stage now uses the authored 1008x764 battle-stage rectangle at
  `(456, 204)`, and the pointer targets the known center of `GridPos(4, 7)`.
- After each real `PickingSystems::Backend` pass, the test reads the actual
  `HitData.position` for `StageProbe` from `HoverMap`. That normalized node-local
  position is combined with the observed `ComputedNode.size`, adjusted by the
  current `UiScale` to the fixed design-stage coordinate system, and passed to
  production `grid_from_stage_point`.
- Both the initial 1920x1080 frame and the resized 1600x1000 frame assert the
  recovered cell is exactly `GridPos(4, 7)`. A generic stage hit with a shifted
  cell therefore fails the test.

Round 2 verification output:

```text
rtk cargo fmt --check
PASS (exit 0)

rtk cargo test --test ui_layout resize_recomputes_scale_before_picking_and_preserves_stage_cell_hits
PASS — 1 passed, 0 failed (7 filtered out)
```

Native title/battle proof remains pending because the Mac is locked. Unknown
untracked file `0` remains untouched, and no asset file was edited.
