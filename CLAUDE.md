# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Scorpius is a single-binary Rust 2024 / Bevy 0.19 desktop tactics game. HPA-632 built the Mission 1 retained combat slice, HPA-635 wrapped it in a linear campaign (Title → VN → briefing → battle → aftermath → upgrades → save), HPA-637 authored Missions 2–3 and the Flanker enemy, HPA-523 authored Missions 4–5 and the Bulwark and Controller enemies, HPA-524 authored Mission 6 and the single-cell Dreadnought boss, and HPA-386 authored Mission 7, the Regent boss, and the campaign ending. The campaign runs authored One→Seven: Mission 7 → Campaign Complete → Return to Title, and a completed Continue resumes at the Ending screen. There is no New Game+. Title's **MISSIONS** opens Mission Select (`GameScreen::MissionSelect`), where any cleared mission replays with the squad's current upgrades: a replay goes straight to its briefing, pays no credits, never advances or saves the campaign (`CampaignSession::replay`), and its victory Continue returns to Mission Select. Post-MVP, Mission 1 gained a road-patrol reinforcement wave on its 128×128 regional map, Missions 2–7 moved onto 64×64 regions with a wave each (`mission/region.rs`), attacks play skippable SRW-style battle cut-ins (`cut_in.rs`), Control's field tips teach each Act 1 mechanic in battle the first time it matters (`field_tips.rs`), pilots spend SP on SRW-style Spirit Commands (`domain/spirit.rs`), and pilots earn XP and levels in campaign battles (`campaign/pilots.rs`): a won campaign battle pays each pilot 30 XP plus 20 per enemy it knocked out (60 for a boss), every 100 XP is a level up to 10, and each level above 1 adds +2 accuracy and +2 evasion (`mission::squad`) and a bigger SP pool. XP is saved in `SquadUpgrades::pilots`; replays pay none, and the aftermath and hangar show it. Sound effects are ElevenLabs-generated Ogg files in `assets/audio/sfx/`; the music and the victory/defeat jingles are synthesized in code (`synth.rs`, `sound.rs`).

## Commands

```bash
cargo run                                                   # launch the game (Title screen)
cargo fmt --check                                           # CI gate
cargo clippy --all-targets --all-features -- -D warnings    # CI gate (warnings are errors)
cargo test --all-targets                                    # unit + integration tests
cargo build --release                                       # CI gate
```

Single test / subset:

```bash
cargo test pure_battle_state_moves_without_bevy             # by name, anywhere
cargo test --lib domain::combat::                           # one inline module
cargo test --test presentation_app                          # only the integration suite
cargo test --release --test balance -- --nocapture         # Act 1 balance report
```

Coverage matches CI (`cargo llvm-cov --all-targets --lcov --output-path lcov.info`).

Every test runs headless — no window, renderer, or Winit — so the full suite works over SSH/CI.
Only `cargo run` and the opt-in `ui_capture` example need a display/GPU.

## Architecture

Three layers, one crate. The dependency direction is strictly `presentation -> mission -> domain`;
**`src/domain/` must never import Bevy.**

### `src/domain/` — canonical rules (plain Rust)

`BattleState` (`battle.rs`) owns everything: board, units, weapons, phase, round, active unit,
committed enemy intents, objectives, result, and RNG. Its fields are private; presentation reads
through accessors (`units()`, `unit()`, `occupant_at()`, `intents()`, `phase()`, `result()`).

Rules are split across files but all `impl BattleState`:

- `battle.rs` — activation lifecycle, movement, reactions, phase guards, terminal checks
- `combat.rs` — `preview_attack` / `attack`, hit & damage math, `DamageSource`
- `enemy.rs` — `begin_round` (enemy movement + intent commitment) and `resolve_enemy_phase`
- `environment.rs` — pushes, collisions, hazards, explosive props
- `spirit.rs` — Spirit Commands: every level-1 player pilot starts with 30 SP (cap 60, +5 each later
  round; each pilot level above 1 adds 4 to the cap, and a pilot starts half full) and spends it on the closed `SpiritCommand` set: Valor (next attack on an enemy deals
  double damage after armor), Alert (the next enemy shot that would hit misses; the roll is still
  drawn, so the RNG stream is unchanged), Guts (repair 12 HP now). Separate from the
  once-per-mission pilot skills (Aegis, Focus, Overdrive). The balance bots never cast them.
- `board.rs`, `model.rs`, `rng.rs` — grid, value types, seeded PRNG

`BattleState::takedowns()` lists the enemies each player pilot knocked out, credited to the pilot
whose attack or counter's events hold the `UnitKnockedOut` (collisions and blasts it caused
included). The campaign turns them into XP; the domain never reads XP back, only
`UnitStats::level` (1 for enemies and fresh pilots), which sizes a pilot's SP pool.

**Every mutating method returns `Result<Vec<BattleEvent>, BattleError>`.** State mutates
synchronously and completely; the returned events are a replay log for presentation, never the
source of truth. Presentation must not infer state from events it hasn't played yet.

### Phase machine

`EnemyPlanning -> Player -> EnemyResolution -> EnemyPlanning` (or terminal `Victory` / `Defeat`).
`resolve_enemy_phase()` resolves committed intents and then calls `begin_round()` itself, so one
"Resolve" click can emit an entire round of events. `app.rs` calls `begin_round()` once at startup
to leave the initial `EnemyPlanning` phase.

**Committed intent is the core invariant:** enemies plan and lock footprint, expected damage, and
hit chance during `begin_round`. Nothing during the player phase — including player movement — may
retarget a committed intent. If player displacement breaks a committed Controller push's live
reach, that intent resolves as its damage roll only (no enemy push, no error); this is the one
sanctioned post-commit degradation. Preserve the invariant when touching `enemy.rs` or movement.

### `src/mission/` — authored data

Mission 1–7 boards, props, units, weapons, openings, and stable IDs (`mission::mission_one::ids`;
`mission::mission_two` through `mission_seven` define their own), all as typed Rust constants.
`mission::enemies` mirrors `mission::squad` with fixed enemy factories for the six regular
archetypes (Rifleman, Striker, Artillery, Flanker, Bulwark, Controller) and their weapons — the
Bulwark is pushable with no displacement immunity, and the Controller's Impulse Projector is
push-only (no status system). Mission 6 adds the Dreadnought boss (HP 40, initiative 40): its
planner picks Graviton Salvo (range 3–6) above half HP and Overload Salvo (range 2–4) at or below,
a threshold that affects future planning only — committed intents stay locked. Mission 7 adds the
Regent boss (initiative 45): Command Barrage (range 3–6 Cross1) above half HP and Rupture Beam
(range 2–4 Single) at or below, the same future-intents-only threshold. Missions 1–7 share
one opening-legality validator
(`assert_opening_plan_is_legal`). No RON/JSON/scripting layer exists; keep new content typed here.
`restart_mission` rebuilds the whole state from a new seed. Mission rules carry a typed
`PrimaryObjective` / `OptionalObjective` plus an `EnemyOpening` plan; only objective boundary
logic lives in the domain (`completed_enemy_round`). Each `MissionDefinition` also authors `tips`: typed `FieldTip`s
(`mission::tips`) whose `TipTrigger` says when Control first explains a mechanic; `TipTrigger::holds`
reads only public `BattleState` accessors. `BattleState::with_reinforcements` holds typed
`Reinforcement` waves off the board: a wave arrives at its round's enemy planning (or as soon as the
deployed enemies are all down), emits `ReinforcementArrived`, holds its arrival cell that planning,
and counts as a living enemy until it arrives. A unit whose arrival cell and every cell reachable from it are taken lands on the nearest open cell anywhere, and waits for the next planning only when the board has none; it never stacks. Mission 1's two-Rifleman road patrol arrives in Round 2. Missions 2–7 build their boards with `region::regional_board`: the authored 9×9 arena sits in the northwest corner of a 64×64 region, walled on its old east and south edges except a gate at (9,8) (so a push east from (8,8) slides onto the road instead of colliding, as at Mission 1's gate), and each mission's `wave()` arrives on the east road at x=14.

### `src/presentation/` — Bevy adapters

Bevy entities are views keyed by domain IDs (`UnitVisual(UnitId)`, `CellVisual(GridPos)`,
`TelegraphVisual`, `IntentLineVisual`, `ReactionVisual`, `PropVisual`); `grid_to_world` maps the
grid to the authored 2.5D stage.

- `interaction.rs` — pure-ish routing (`route_cell_click`, `execute_command`, `update_hover_preview`)
  that takes `&mut BattleState` and returns events. Directly unit-testable, and where most
  integration tests hook in.
- `playback.rs` — drains `BattleEventQueue` one event at a time on a `Timer`, holding
  `EventPlayback::input_locked` until the queue empties. Adding a `BattleEvent` variant means
  giving it a duration and a visual in `event_duration` / `play_battle_events`. Waves still off
  the board get hidden tokens at rebuild; `ReinforcementArrived` reveals them and cuts the regional
  camera to the arrival and back.
- `cut_in.rs` — SRW-style battle cut-ins. `stage_cut_ins` (after `play_battle_events`) holds the
  first `AttackRolled` of each strike for `CUT_IN_SECONDS` and builds a `CutInSnapshot` from the
  events still queued behind it; `animate_cut_in` lays out the band. Presentation only: no domain
  state or RNG changes. They play only when the `CutInSettings` resource exists and is enabled,
  so bare test apps keep map-speed playback.
- `field_tips.rs` — Control's comms band under the stage. `stage_field_tips` (after the input
  systems) queues each authored tip on the first idle Player-phase frame its trigger holds, once per
  battle visit: `enter_battle` resets `FieldTips`; a restart forgets undismissed tips, which queue again when their trigger next holds, but keeps what was taught. Presentation only, never
  modal. The `TipSettings` resource gates it; the `ui_capture` fixtures turn it off unless a fixture
  uses `CaptureAction::ShowTip`.
- `synth.rs` / `sound.rs` — 18 of the 20 typed `Sfx` effects are mono Ogg files in `assets/audio/sfx/`
  (generated with ElevenLabs, then trimmed and loudness-matched; `Sfx::asset_path` names each).
  `Sfx::Victory` / `Sfx::Defeat` play ~5 s `Jingle`s built from the battle and campaign themes. The
  jingles and the two looping music `Track`s are synthesized in code at startup (`synth.rs` is plain
  Rust), and `sound.rs` wraps them as the `SynthSound` audio source. `sound.rs` plays the cues playback, cut-ins and `Button` clicks leave in
  `SoundCues`, and loops the Theme or Battle track for the current screen (battle music stops once a
  finished battle's events have played). A cut-in holds its strike's hit/miss sound until the blow
  lands. Bare test apps without `SoundPlugin` stay silent. `N` mutes on any screen.
- `sync.rs` — reconciles marker entities against current state each frame (spawn/despawn diffing).
  `cell_highlights` projects the armed command onto the board (blue move cells, amber weapon reach
  from `BattleState::weapon_reach` with pulsing targets `preview_attack` accepts, mint Aegis allies,
  a white rim on the clickable hovered cell) and paints each cell's `CellOverlayVisual`; terrain
  insets are never tinted.
- `prompt.rs` — the one-line next-step prompt above the stage (`battle_prompt`): what the player
  can do now and which key does it. Hidden outside an idle Player phase.
- `ui.rs` — native Bevy UI. `HudSnapshot::from_battle` derives all HUD text from state, so HUD logic
  is testable without a renderer; the sidebar, Inspector, Preview, Threat, Log, and Result regions
  are projections of typed snapshots.
- `battle_menu.rs` — the icon-led Root → Weapons/Stances menus, guarded command routing, and
  accessible icon-only weapon rows.
- `layout.rs` — the fitted fixed-pixel canvas and authored 1008×764 2.5D battle stage. `battlefield.rs`
  and `sync.rs` render the stage with native Bevy UI nodes and `Camera2d`.
- `assets.rs`, `theme.rs` — checked-in UI atlas/font assets and source-shaped icon rectangles;
  do not add a parallel icon registry or art pipeline.

### Current presentation controls

The battle sidebar opens with **MOVE**, **ATTACK**, **STANCE**, **SKILL**, **SPIRIT**, and **WAIT**.
**ATTACK** opens the three typed weapon rows; **STANCE** opens Counter, Guard, and Evade; **SPIRIT**
(or `Q`) opens Valor, Alert, and Guts with their SP costs. **WAIT** finishes
the active unit, and **RESOLVE** becomes available after surviving units finish. `Escape` invokes
the existing Cancel action for menus and targeting. `R` invokes the guarded Restart action during
an idle Player phase or after defeat; it remains guarded during activation/loading/playback/planning/
resolution/victory/pending transitions. Victory exposes Continue. During a battle cut-in, `Space`,
`Enter`, `Escape`, or a left click skips it, and `V` turns cut-ins off or on at any time. `N` turns sound off or on on any screen. During an
idle Player phase, `Enter` dismisses the field tip on screen, and `H` turns field tips off or on. The campaign **SKIP** button is created only for
`GameScreen::PreMissionStory`. On the story and aftermath dialogue screens, `Enter` or `Space`
advances a line like the Next button. Keep these bindings in the existing command/action types.

Regional boards (larger than 9×9) add camera controls: **left-drag**, **arrow keys**,
**middle-drag**, and a trackpad's two-finger scroll pan, and a **minimap** click focuses the cell
under the cursor; a left press turns into a pan only past a click's jitter (`MapDrag`), and the
release that ends a pan clicks no cell. A mouse **wheel**, a trackpad **pinch**, and the **−/+** buttons zoom,
and **Center**/`Home` re-center on the active unit, then the inspected unit, then the first
living player (`focus_unit` in `map_view.rs`). The icon buttons carry the typed `MapAction` component (a
sibling of `CommandAction` in `interaction.rs` and `MenuAction` in `battle_menu.rs`); continuous pan/zoom is polled in
`navigate_map` because it is camera input, not a battle command. Map navigation stays live while
event playback locks battle input — playback effects (damage numbers, impact icons) live under
`BattleMap` in map space, so they pan and clip with the stage.

The opt-in native renderer fixture is built with:

```bash
cargo build --features ui-capture --example ui_capture
target/debug/examples/ui_capture --scenario battle-active-vanguard \
  --size 1920x1080 --seed 7 --time-ms 0 \
  --output target/ui-capture/battle-active-vanguard.png
```

`--time-ms` advances presentation playback after fixture setup; use
`--size 1600x1000 --scenario letterbox-1600x1000` for the letterbox case. The
fixture uses the production UI tree and an isolated temporary save.

### Event flow

`interaction`/`playback` call a domain method → returned `Vec<BattleEvent>` is pushed into
`BattleEventQueue` → `play_battle_events` pops one per tick, animating and locking input → HUD reads
`BattleRuntime` directly. `app.rs` wires the whole system order explicitly with `.chain()` and
`.after(...)`; new systems need a deliberate slot there (restart → rebuild → reconcile → sync →
playback → input → HUD).

## Determinism

`BattleRng` is a seeded splitmix64. `app.rs` seeds from the clock at launch, but tests construct
`mission_one(<fixed seed>)` and assert on exact rolls — `rng.rs` pins concrete outputs for known
seeds. Changing the PRNG, or the *number/order* of `roll_percent()` calls in a resolution path,
breaks seeded tests; that's intended signal, not flakiness.

## Testing conventions

- Domain rules: inline `#[cfg(test)] mod tests` in each `src/domain/*.rs`.
- Presentation seams: `tests/presentation_app.rs`, building a bare `App::new()` with only the
  resources under test (see `presentation_fixture_app`) — never `DefaultPlugins`.
- `BattleState::viability_fixture()` is a minimal two-unit state for tests that don't need Mission 1.
- `tests/balance.rs` plays the campaign with three scripted policies (Novice, Reader, Tactician)
  through the public domain API and asserts Act 1 difficulty guard rails. Tuning a weapon, unit, or
  opening moves its numbers; rerun it with `--nocapture` and record the change in
  `docs/validation/act-1-balance.md`.

## Constraints (from the HPA-632 spec)

- One application crate — not a workspace, engine crate, or plugin suite.
- `bevy = "0.19"` pinned, `Cargo.lock` committed. No physics engine, no second UI framework.
- Authored 2.5D rendering uses `Camera2d`, native Bevy UI nodes, `UiPickingCamera`, and the
  checked-in UI/board atlas assets.
- No data-file authoring format, scripting, behavior trees, generic status/ability framework,
  backend, or persistence.

## Reference docs

- `docs/superpowers/specs/2026-08-23-hpa-632-*-design.md`, `2026-08-26-hpa-635-*.md`,
  `2026-08-28-hpa-637-*.md`, `2026-08-30-hpa-523-*.md`, `2026-08-31-hpa-524-mission-6-dreadnought-design.md`,
  `2026-09-02-hpa-386-mission-7-mvp-closeout-design.md` — design specs and rules of record
- `docs/superpowers/plans/2026-08-23-hpa-632-*.md` and the 635/637/523/524/386 siblings — implementation plan ledgers
- `docs/validation/hpa-632.md`, `docs/validation/hpa-635.md`, `docs/validation/hpa-637.md`,
  `docs/validation/hpa-523.md`, `docs/validation/hpa-524.md`, `docs/validation/hpa-386.md` —
  playtest and viability evidence
- `docs/validation/act-1-balance.md` — the headless Act 1 balance pass; its tuned values supersede
  the spec numbers it lists

Commits follow Conventional Commits (`feat:`, `ci:`, `docs:`, `chore:`).
