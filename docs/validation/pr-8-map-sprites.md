# PR #8 (SRW map sprites) validation

Validation scope: the `anime-battle-token-art` branch (open PR #8), Tasks 1–6 of
[the SRW map-sprites plan](../superpowers/plans/2026-09-16-srw-map-sprites.md).
Code gates were recorded at commit `5ef1a32` (`feat: keep map-sprite scale
effects anchored at the feet`); the Task 6 commit itself adds only docs and
one `.gitignore` line (`/test_output`).
All steps were executed locally on macOS 26.6.2 / Apple M1 Pro with a real
display and Metal GPU — nothing was skipped for lack of display.

## Map-sprite art install (asset commit)

The four corrupt `assets/ui/map/*.png` blobs (art commit `84a0318`) were
replaced with the user-supplied, ChatGPT-generated SD anime mecha art,
normalized with Python 3 + Pillow 12. Sources were already RGBA; each was
trimmed to its alpha bbox, scaled to fit 256×256 preserving aspect (feet
filled to the bottom edge), and pasted bottom-center onto a transparent
256×256 RGBA canvas. `file(1)` confirms all four as
`PNG image data, 256 x 256, 8-bit/color RGBA`; each was read back visually to
confirm a transparent surround, feet on the bottom edge, and a legible mech.

Classification (source → role, one line each):

- `(1)` → `vanguard` — heroic white/blue mech with glowing energy sword and
  shield; reads as the player melee leader.
- `(2)` → `enemy` — dark grey mass-production palette with red mono-eye,
  grunt rifle and shield; the clear hostile of the set.
- `(3)` → `gunner` — bulky green/white mech with a giant gatling cannon and
  shoulder missile pods; the heavy ranged support.
- `(4)` → `interceptor` — slim red/white mech with energy claws and spiky
  wings; the light fast skirmisher.

Each source fit exactly one role; the closest call was `(2)` (it carries a
rifle, but `(3)` is unmistakably the heavy gunner and `(2)`'s dark grunt
palette is the enemy).

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

## E2E suite — PASS (was failing on the corrupt-asset gate)

```text
cargo test --features e2e --test e2e -- --test-threads=1
PASS — critical_flow_boots_to_mission_one_and_moves_vanguard (6.96s)
```

History: with the corrupt `84a0318` sprite bytes this suite failed because the
asset readiness gate (`AssetLoadStatus::Failed` → "LOCKED / ASSET LOAD FAILED"
banner) locked all input — `Invalid PNG signature` was reported for all four
map sprites (`test_output/scorpius-87386-1/` holds that run's artifacts). With
the normalized sprites installed, the real rendered E2E boots Title → New
Game → skip VN → briefing → battle and drives the Vanguard move to (4, 8)
through BRP-inspected node positions. One cold-boot race was observed once
(the first run clicked `campaign.new_game` during startup and timed out
waiting for the VN skip button; `test_output/scorpius-30349-1/` holds that
run's artifacts). Root cause: an existence-before-layout click miss —
`wait_for` proves only entity existence, so a fire-once nav click can land
in the spawn frame before `ui_layout` computes the button's
`UiGlobalTransform` and then miss. Fixed in the e2e harness usage by the
`click_through` self-heal (`test: self-heal e2e navigation clicks lost to
the cold-boot layout race`), which re-clicks each nav hop until the next
screen's marker appears; the battle section already used the same
click-retry shape.

## Native captures — PASS (all three scenarios screenshot)

Renderer fixture run with a real window on Metal (display and GPU available):

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

All three exits 0 and wrote screenshots (previously the asset gate aborted
with `capture failed: UI asset ui/map/vanguard.png did not load` before any
screenshot was written). Visual verdicts from reading the captures:

- `battle-idle` — PASS. The three player mechs are distinct at 96px (Gunner
  with gatling cannon, Vanguard with sword + shield, Interceptor claws),
  standing bottom-anchored on their cells; the four enemy grunts read clearly
  with HP bars, archetype intent lines, and the `LOCKED 4` planning pill (the
  normal enemy-planning input lock at `--time-ms 0`). No card chrome on the
  battlefield.
- `battle-active-vanguard` — PASS. Selection overlay (cyan move/attack tiles,
  yellow origin), Vanguard sidebar menu and inspector readable, enemy intent
  cards (Striker 83%, Artillery 90%) with archetype glyphs; overlays do not
  obscure the sprites.
- `battle-playback` — PASS. Event log (`LOCKED INTENT`/`MOVING`, "Resolving
  committed events…") with Vanguard interpolated mid-move at 150 ms; feet
  stay grounded on tile shadows and depth ordering against the dark props is
  sane.

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
