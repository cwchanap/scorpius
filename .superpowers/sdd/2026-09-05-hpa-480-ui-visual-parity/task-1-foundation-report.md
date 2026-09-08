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
- The two pre-existing local `text_font` helpers in `ui.rs` and
  `campaign_ui.rs` remain until the screen/theme cutover. The new exact-face
  helper and asset readiness path are in `theme.rs`; migrating those legacy
  call sites belongs with the later screen cutover and was intentionally kept
  outside this foundation-only scope.
