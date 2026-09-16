# PR #8 (SRW map sprites) validation

Validation scope: the `anime-battle-token-art` branch (open PR #8), Tasks 1–6 of
[the SRW map-sprites plan](../superpowers/plans/2026-09-16-srw-map-sprites.md).
Code gates were recorded at commit `5ef1a32` (`feat: keep map-sprite scale
effects anchored at the feet`); the Task 6 commit itself adds only docs.
All steps were executed locally on macOS 26.6.2 / Apple M1 Pro with a real
display and Metal GPU — nothing was skipped for lack of display.

## Automated gates

```text
cargo fmt --check
PASS (exit 0)

cargo clippy --all-targets --all-features -- -D warnings
PASS — no errors/warnings (exit 0)

cargo test --all-targets
PASS — 305 passed, 0 failed across all 10 test binaries
(188 lib + 0 main + 28 campaign_flow + 1 campaign_model + 25 campaign_persistence
+ 29 presentation_app + 7 ui_interaction + 13 ui_layout + 14 ui_snapshots;
e2e compiles empty without the feature)

cargo build --release
PASS (exit 0, 5m48s)
```

## E2E suite — FAILED: renderer gate rejects corrupt map-sprite PNGs

```text
cargo test --features e2e --test e2e -- --test-threads=1
FAIL — critical_flow_boots_to_mission_one_and_moves_vanguard
(wait_until timed out after 60s; total 75s)
```

The game boots and renders the full battle stage on Metal, but the production
asset readiness gate (`AssetLoadStatus::Failed` → "LOCKED / ASSET LOAD FAILED"
banner) locks all input, so the movement flow can never be clicked through.
Root cause, from the child stderr log:

```text
Failed to load asset 'ui/map/vanguard.png' with asset loader
'bevy_image::image_loader::ImageLoader': Could not load texture file: Error
reading image file ui/map/vanguard.png: failed to load an image: Format error
decoding Png: Invalid PNG signature.
```

The same `Invalid PNG signature` error is reported for all four map sprites
(`ui/map/vanguard.png`, `ui/map/gunner.png`, `ui/map/interceptor.png`,
`ui/map/enemy.png`). The committed bytes in `assets/ui/map/` (art commit
`84a0318`, which predates plan Task 1) do not begin with the PNG magic
signature — none of the four files is decodable by `image` or identifiable by
`file(1)`, and the git object store contains no earlier non-corrupt version of
any of them. Every test in Tasks 1–5 is headless (fixture `Handle::default()`
handles), so the corruption was invisible until a real renderer loaded the
catalog. This is a pre-existing blocker on the branch, not a Task 6
regression; it must be fixed by committing valid 96×96 sprite PNGs at those
four paths before PR #8 can pass its capture/E2E evidence gates.

Failure artifacts (local run, not committed): `test_output/scorpius-87386-1/`
— `screenshot.png` shows the rendered stage with the
`LOCKED / ASSET LOAD FAILED ui/map/vanguard.png` banner; `stderr.log` holds
the loader errors quoted above.

## Native captures — FAILED: same asset-gate root cause

Renderer fixture attempted with a real window (display and GPU available, so
this is a recorded failure, not SKIPPED-NO-DISPLAY):

```bash
cargo build --features ui-capture --example ui_capture   # PASS
target/debug/examples/ui_capture --scenario battle-idle --size 1920x1080 \
  --seed 7 --time-ms 0 --output target/ui-capture/battle-idle.png
target/debug/examples/ui_capture --scenario battle-active-vanguard \
  --size 1920x1080 --seed 7 --time-ms 0 \
  --output target/ui-capture/battle-active-vanguard.png
target/debug/examples/ui_capture --scenario battle-playback --size 1920x1080 \
  --seed 7 --time-ms 150 --output target/ui-capture/battle-playback.png
```

All three scenarios exit 1 with `capture failed: UI asset ui/map/vanguard.png
did not load` (the `AssetLoadStatus::Failed` path in `capture_assets_ready`
aborts before any screenshot is written), so **no capture files exist** at
`target/ui-capture/battle-idle.png`,
`target/ui-capture/battle-active-vanguard.png`, or
`target/ui-capture/battle-playback.png`. Per the no-faked-evidence rule the
captures are left unattempted-with-placeholder art; they must be re-run after
valid sprite PNGs land.

## E2E click geometry — ~8px horizontal clearance (future-geometry warning)

The E2E click on `battle.cell.4.8` sits ~8px horizontally from Gunner's 96px
root: Gunner is authored at (3,8) (stage x = 504 + (3−8)·56 = 224), so its
bottom-centered 96px root spans stage x ∈ [176, 272]; the (4,8) cell center —
where the click lands — is stage x = 504 + (4−8)·56 = 280, i.e. 8px right of
the root's right edge. With Task 4's token-hit conversion, a click that lands
on Gunner's root resolves to Gunner's underlying diamond (3,8) instead of
(4,8), which would fail the E2E movement assertion. Any future change that
widens the unit roots, shifts sprites off tile centers, or moves Gunner's
authored cell can consume this clearance — re-check this margin when touching
`MAP_UNIT_WIDTH`/placement or Mission 1 openings.

## Task 5 scale-formula ruling (record of record)

`set_unit_scale` in `src/presentation/playback.rs` uses the full
`(1.0 - scale) * MAP_UNIT_HEIGHT` vertical-offset factor, not the `* 0.5`
factor sketched in the PR body. Bevy UI scales nodes around their center; the
0.5 factor pins the token's center and lifts the feet (at scale 0.9 the feet
lift 4.8px, and the knockout shrink would hover ~47px above the shadow). The
PR body's own acceptance criteria — "pulses and knockout shrink keep the feet
grounded on the shadow", "bottom edge fixed" — require the full factor, and
the full factor is what shipped in `5ef1a32`
(`unit_scale_effects_keep_the_ninety_six_pixel_bottom_edge_fixed`). If a
future ruling prefers center-pinned scaling, the change is the single `0.5`
multiplication in `set_unit_scale`.
