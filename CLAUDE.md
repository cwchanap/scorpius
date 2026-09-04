# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Scorpius is a single-binary Rust 2024 / Bevy 0.19 desktop tactics game. HPA-632 built the Mission 1 retained combat slice, HPA-635 wrapped it in a linear campaign (Title → VN → briefing → battle → aftermath → upgrades → save), HPA-637 authored Missions 2–3 and the Flanker enemy, HPA-523 authored Missions 4–5 and the Bulwark and Controller enemies, HPA-524 authored Mission 6 and the single-cell Dreadnought boss, and HPA-386 authored Mission 7, the Regent boss, and the campaign ending. The campaign runs authored One→Seven: Mission 7 → Campaign Complete → Return to Title, and a completed Continue resumes at the Ending screen. The MVP has no mission select, no New Game+, and no dedicated battle-animation scene.

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
```

Coverage matches CI (`cargo llvm-cov --all-targets --lcov --output-path lcov.info`).

Every test runs headless — no window, renderer, or Winit — so the full suite works over SSH/CI.
Only `cargo run` needs a display.

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
- `board.rs`, `model.rs`, `rng.rs` — grid, value types, seeded PRNG

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
logic lives in the domain (`completed_enemy_round`).

### `src/presentation/` — Bevy adapters

Bevy entities are views keyed by domain IDs (`UnitVisual(UnitId)`, `CellVisual(GridPos)`,
`TelegraphVisual`, `IntentLineVisual`, `ReactionVisual`, `PropVisual`); `grid_to_world` maps the
grid to world space.

- `interaction.rs` — pure-ish routing (`route_cell_click`, `execute_command`, `update_hover_preview`)
  that takes `&mut BattleState` and returns events. Directly unit-testable, and where most
  integration tests hook in.
- `playback.rs` — drains `BattleEventQueue` one event at a time on a `Timer`, holding
  `EventPlayback::input_locked` until the queue empties. Adding a `BattleEvent` variant means
  giving it a duration and a visual in `event_duration` / `play_battle_events`.
- `sync.rs` — reconciles marker entities against current state each frame (spawn/despawn diffing).
- `ui.rs` — native Bevy UI. `HudSnapshot::from_battle` derives all HUD text from state, so HUD logic
  is testable without a renderer.
- `battlefield.rs`, `assets.rs` — scene setup and the single checked-in glTF.

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

## Constraints (from the HPA-632 spec)

- One application crate — not a workspace, engine crate, or plugin suite.
- `bevy = "0.19"` pinned, `Cargo.lock` committed. No physics engine, no second UI framework.
- Orthographic `Camera3d`, Bevy mesh picking, native Bevy UI, checked-in glTF only.
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

Commits follow Conventional Commits (`feat:`, `ci:`, `docs:`, `chore:`).
