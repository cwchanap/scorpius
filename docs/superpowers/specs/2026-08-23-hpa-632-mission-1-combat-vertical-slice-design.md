# HPA-632 Mission 1 Combat Vertical Slice Design

**Date:** 2026-08-23  
**Status:** Approved direction; implementation design  
**Baseline:** Live Linear issue HPA-632 and the Scorpius project description, both fetched 2026-08-23

## Outcome

Create one desktop-first Rust 2024 application using Bevy 0.19 that starts directly in a retained, playable Mission 1. The mission must prove both halves of the Scorpius hypothesis:

1. Bevy can present a maintainable orthographic 3D battlefield with native screen-space UI and checked-in glTF art.
2. Reading and manipulating committed enemy attacks is more interesting than choosing only the highest-damage player attack.

The delivery boundary is one HPA-632 branch and pull request. The early Bevy viability checkpoint is an internal milestone on that same branch, not a throwaway spike or prerequisite PR.

## Chosen Architecture

Use a domain-first, phase-oriented application with one canonical `BattleState` and a thin Bevy adapter.

- Plain Rust owns grid occupancy, units, weapons, movement, attacks, RNG, enemy intents, reactions, environment rules, objectives, and restart.
- Explicit transition functions mutate `BattleState` and return small, ordered battle events for presentation.
- Bevy resources hold the canonical state and transient interaction state. Bevy entities carry stable domain IDs and render or forward input; they do not independently own combat truth.
- Bevy systems are grouped around loading, player input, state-to-scene synchronization, battle-event presentation, and native UI. Combat rules do not become a long chain of ECS events.
- Mission 1 content remains concrete typed Rust data. There is no mission format, editor, registry, scripting language, behavior tree, dependency-injection layer, or generic ability/status framework.

### Alternatives considered

1. **ECS-led combat state:** initially convenient for rendering, but it makes rules dependent on schedules and entity queries, weakens pure tests, and conflicts with the ticket's canonical-state constraint.
2. **Command log/reducer architecture:** useful for replay and rollback, but those are explicitly out of scope. HPA-632 needs direct, understandable transitions rather than replay infrastructure.

## Project Shape

Use one application crate named `scorpius`, with `Cargo.lock` committed.

```text
src/
  main.rs                    # desktop app entry point
  app.rs                     # Bevy plugin composition and top-level app state
  domain/
    mod.rs
    battle.rs                # BattleState, phases, objectives, restart
    board.rs                 # GridPos, occupancy, movement/path checks
    combat.rs                # previews, attack resolution, RNG, knockout
    environment.rs           # push, collision, explosive, hazard
    enemy.rs                 # Mission 1 deterministic positioning/intents
    model.rs                 # IDs, units, weapons, reactions, events/errors
  mission/
    mod.rs
    mission_one.rs           # all authored Mission 1 data and layout
  presentation/
    mod.rs
    assets.rs                # glTF handles and load-state gate
    battlefield.rs           # camera, board, visuals, highlights, telegraphs
    interaction.rs           # picking and player command intent
    sync.rs                  # canonical state -> entity transforms/visibility
    ui.rs                    # native Bevy HUD, command bar, previews, results
assets/
  models/mission_one.gltf    # checked-in original low-poly scenes
.github/workflows/ci.yml
docs/validation/hpa-632.md   # checkpoint and final manual-play evidence
```

Files may be split further only when a file develops two concrete responsibilities. Do not introduce a Cargo workspace or reusable engine/plugin crate.

## Application and Battle Flow

The application has only the states required by this ticket:

1. **Loading:** load the checked-in glTF and create native UI. A visible loading/error overlay prevents a blank or half-interactive board.
2. **Battle:** run Mission 1 through explicit domain phases.
3. **Result:** show victory/bonus or failure and allow immediate restart.

Mission 1 starts automatically after assets load. There is no title screen, save, story scene, upgrade screen, or campaign transition; HPA-635 owns those.

Each round follows this order:

1. Clear last round's player reaction stances.
2. Surviving enemies perform deterministic positioning.
3. Each surviving enemy commits one attack profile and fixed legal footprint. The footprint may be empty when no player is currently in range.
4. The player activates Vanguard, Gunner, and Interceptor in any order.
5. During one unit's activation, Move and Action are each available at most once and may occur in either order. Finishing early explicitly skips unused allowances.
6. The player chooses Counter, Guard, or Evade, then finishes that unit. A finished unit cannot be reopened that round.
7. After all surviving player units finish, the player confirms enemy resolution.
8. Committed enemy attacks resolve in authored initiative order without retargeting.
9. Surviving Counter units may answer the attack that hit them; enemies never counter counters.
10. Check victory/failure, otherwise begin the next round.

The player may inspect any unit and threat at any time, but only the currently activating player unit can issue commands. This preserves free activation order without introducing cross-unit action interleaving.

## Mission 1: Turnabout at Relay Nine

### Objective and duration

- **Primary:** eliminate all four enemies.
- **Optional — Turnabout:** before victory, cause at least one enemy to take damage from either an enemy's committed attack or an environmental event (collision, hazard, or explosive object).
- **Failure:** all three player mechs are knocked out.
- **Turn limit:** none. The encounter already has pressure from committed attacks and EN.
- **Target duration:** 15–20 minutes for a first successful playthrough and under 10 minutes once understood.

The optional result is stored in the mission result object for HPA-635 to consume later. HPA-632 does not award credits.

### Board

Use a 9×9 square grid. Coordinates are zero-based with `(0, 0)` at the upper-left from the authored tactical view. The board uses orthogonal movement and Manhattan range only.

The following diagram shows positions after the authored first-round enemy movement and before intents appear:

```text
    x: 0 1 2 3 4 5 6 7 8
y0     . . . . A . . . .
y1     . . # . . . # . .
y2     . . . . . . . . .
y3     . . . . . . . . .
y4     . # . . . . . # .
y5     . . R # . # R . .
y6     . . ~ . S . X . .
y7     . . . . V . . . .
y8     . . . G . I . . .
```

Legend:

- `V`, `G`, `I`: Vanguard, Gunner, Interceptor
- `R`: Rifleman; `S`: Striker; `A`: Artillery
- `#`: blocking terrain
- `~`: damaging hazard
- `X`: explosive object

The opening plan is deliberately authored:

- The Striker advances to `(4, 6)` and commits against the Vanguard at `(4, 7)`.
- The Artillery commits a cross-shaped blast centered on the Vanguard. Its footprint includes `(4, 6)`, so vacating the Vanguard's cell lets the later blast threaten the Striker without retargeting.
- The two Riflemen advance into the side lanes and commit readable ranged attacks.

This makes the core hook visible in round one without a tutorial framework. Later rounds use the same small deterministic archetype rules rather than scripted per-turn sequences.

### Player squad

All values are authored in Mission 1 data and may be tuned after the required playtest without changing architecture.

| Mech | HP | Armor | Move | Accuracy | Evasion | EN | Role |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Vanguard | 20 | 3 | 3 | 78 | 5 | 7 | Durable short-range control |
| Gunner | 12 | 1 | 2 | 86 | 10 | 9 | Fragile long-range precision |
| Interceptor | 15 | 1 | 4 | 82 | 20 | 8 | Mobile utility and displacement |

Each mech has exactly three weapons:

| Mech | Weapon | Range | Shape | Damage | Hit mod | Crit | EN | Push | Counter weapon |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| Vanguard | Pile Lance | 1 | Single | 8 | +10 | 15% | 0 | — | Yes |
| Vanguard | Repulsor Ram | 1 | Single | 5 | +15 | 5% | 2 | 1 | No |
| Vanguard | Anchor Cannon | 2–3 | Single | 6 | +0 | 10% | 3 | 1 | No |
| Gunner | Rail Rifle | 3–6 | Single | 7 | +15 | 20% | 0 | — | Yes |
| Gunner | Burst Missile | 2–5 | Cross 1 | 5 | +5 | 10% | 3 | — | No |
| Gunner | Overcharge Shot | 2–6 | Single | 10 | −15 | 25% | 5 | — | No |
| Interceptor | Arc Blade | 1 | Single | 6 | +15 | 15% | 0 | — | No |
| Interceptor | Pulse Carbine | 2–4 | Single | 4 | +20 | 10% | 1 | — | Yes |
| Interceptor | Vector Pulse | 1–2 | Single | 4 | +10 | 5% | 3 | 1 | No |

Push weapons require attacker and target to share a row or column, so the away direction is unambiguous. Player attacks do not damage allied mechs. Area attacks can damage multiple enemies and destructible objects, with each affected combatant resolved once.

### Enemies

| Archetype | Count | HP | Armor | Move | Accuracy | Evasion | Weapon |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Rifleman | 2 | 9 | 1 | 2 | 72 | 5 | Service Rifle: range 2–4, single, damage 5, hit +0, crit 5% |
| Striker | 1 | 12 | 2 | 2 | 78 | 10 | Shock Claw: range 1, single, damage 7, hit +10, crit 10% |
| Artillery | 1 | 10 | 1 | 1 | 90 | 0 | Siege Mortar: range 3–8, Cross 1, damage 6, hit +5, crit 5% |

Enemy weapons do not use EN in this slice.

Enemy positioning is small and deterministic:

- Riflemen seek the nearest legal cell that leaves a player in their range band.
- The Striker follows the shortest orthogonal path toward the nearest player until it can commit Shock Claw.
- The Artillery stays put while it can fire; otherwise it moves one cell along its authored central lane.
- Targets first maximize the number of players in the attack footprint, then use the fixed tie-break order Vanguard, Gunner, Interceptor, followed by coordinate order. If no player is in range, the enemy commits against the legal empty footprint closest to the nearest player.
- Movement ties use stable coordinate order. A small breadth-first search over the 9×9 board is acceptable; no behavior tree or search-heavy evaluation is introduced.

Every surviving enemy therefore owns exactly one committed intent. An empty footprint has no intended occupant and displays the weapon's base damage and base hit value; it can still hit a unit that later enters the footprint. The authored opening guarantees four occupied threats.

## Canonical Domain Model

`BattleState` owns:

- board dimensions and terrain/prop cells
- units keyed by stable `UnitId`
- occupancy derived and validated by domain transitions
- per-unit Move, Action, finished, and reaction state
- current battle phase and round
- committed enemy intents in stable resolution order
- objective progress and terminal mission result
- a small seedable combat RNG

Important value types include `GridPos`, `UnitId`, `WeaponId`, `Faction`, `UnitState`, `WeaponSpec`, `AttackProfile`, `AttackIntent`, `Reaction`, `ObjectiveProgress`, `BattlePhase`, `BattleEvent`, and `BattleError`.

Public transition functions are explicit and narrow:

- `begin_round`
- `begin_activation`
- `move_unit`
- `preview_attack`
- `attack`
- `choose_reaction`
- `finish_activation`
- `resolve_enemy_phase`
- `restart_mission`

Transitions reject invalid phase, unit, range, occupancy, allowance, target, or EN state before mutation. EN is deducted exactly once only after a player attack is accepted. A rejected command leaves canonical state unchanged and returns a user-displayable error.

Transitions return ordered `BattleEvent` values such as movement, attack roll, damage, displacement, collision, explosion, knockout, intent cancellation, counter, objective progress, and mission completion. These events are a local presentation handoff, not a global application event bus or persisted command log.

## Combat Resolution

### Preview and attack profile

For a unit target:

```text
hit chance = clamp(attacker accuracy + weapon hit modifier - defender evasion, 5, 95)
normal damage = max(1, weapon base damage - defender armor)
critical raw damage = weapon base damage + floor(weapon base damage / 2)
critical damage = max(1, critical raw damage - defender armor)
```

Guard then reduces incoming post-armor damage by 3, to a minimum of 0. Evade adds 25 evasion before the hit-chance clamp. Props are automatically hit, do not evade, and cannot be critically hit.

The same pure calculation produces both previews and resolution inputs. There is no separate UI approximation.

An `AttackIntent` snapshots attacker, weapon, origin, fixed unique footprint, attack profile, intended occupant when present, and the intended-occupant preview when present. Empty footprints expose the profile's base damage and base hit value. At resolution an intent never chooses a new target cell. It checks the current occupant of each committed cell and applies the snapshotted profile against that occupant's current armor/evasion/reaction.

Enemy attacks may damage either faction. Therefore:

- an empty committed cell is visibly struck but damages nothing
- a different player occupying the cell may be hit
- an enemy occupying the cell may be hit and can satisfy Turnabout
- a knocked-out attacker cancels its unresolved intent

### RNG

Only hit/evasion and critical checks consume RNG. Use one small deterministic PRNG owned by `BattleState`, constructed from a `u64` seed. Production creates a fresh seed when a mission starts; tests pass known seeds. This is not exposed as replay infrastructure.

Each affected combatant consumes one hit roll; a hit then consumes one critical roll. UI previews show percentages, not predicted rolls.

### Knockout

HP is clamped at zero. A unit at zero HP is knocked out for the mission, immediately stops occupying its cell, cannot act or react, and cancels any unresolved intent it owns. Knockout has no campaign consequence in HPA-632.

## Reactions

Each player mech must choose exactly one stance before finishing its activation:

- **Counter:** after surviving a hit from an enemy, fire the mech's designated counter weapon if the attacker is in legal range/line and sufficient EN remains. The normal EN cost is deducted once. A miss does not refund EN.
- **Guard:** reduce every incoming post-armor damage instance by 3 until the next player phase; never counter.
- **Evade:** add 25 evasion to every incoming attack until the next player phase; never counter.

Counters do not consume Move or Action. They cannot trigger another counter, do not chain, and cannot target anyone except the current attacker. For an area intent that hits a mech once, that mech receives at most one counter opportunity for that intent.

## Board Manipulation and Environment

### Push 1

After a successful hit from a Push 1 weapon, resolve one cell directly away from the attacker.

- Empty legal destination: move the target.
- Blocking terrain, explosive object, another unit, or board edge: do not move; deal 3 collision damage to the pushed unit only.
- Hazard destination: move first, then resolve the hazard once.

Push never chains into another push and never moves more than one cell.

### Damaging hazard

The hazard deals 3 direct damage, ignoring armor, once when a movement or displacement transaction ends on its cell. Remaining on the tile across phases does not deal repeated damage; leaving and later re-entering creates a new transaction. One domain transition records whether the hazard was resolved, preventing frame- or event-driven duplication.

### Explosive object

The explosive object has 4 HP, blocks movement, and is automatically hit by player or enemy footprints. The first transition that reduces it to zero marks it destroyed and applies one 4-damage direct event to occupants in its own and four orthogonally adjacent cells. It cannot explode twice and does not create a generic chain-reaction system.

Collision, hazard, and explosion damage may affect either faction and can satisfy Turnabout when an enemy is damaged.

## Bevy Presentation Boundary

### Scene and assets

- Render a true 3D board through an orthographic `Camera3d` at an isometric angle.
- Load checked-in low-poly mech, enemy, blocking-terrain, explosive, hazard, and impact-marker scenes from `assets/models/mission_one.gltf` through Bevy's `AssetServer` and glTF scene labels.
- Use simple PBR or unlit materials. Do not add a custom renderer.
- Invisible or minimally visible per-cell meshes may be created in Bevy for picking and highlights; authored battlefield objects remain glTF-backed.
- Stable ID components such as `UnitVisual(UnitId)` and `CellVisual(GridPos)` connect entities to canonical state.

The glTF is original lightweight placeholder production art built from low-poly primitives and committed with the project. It is intentionally replaceable later without changing domain rules. No Blender/editor dependency or reusable asset pipeline is added to the repository.

### Picking and commands

Use Bevy's built-in mesh picking on one pickable mesh per logical cell. Pointer hover updates inspection/highlighting only. Pointer clicks become high-level interaction intents:

- select an unactivated player unit
- choose a legal destination while Move is armed
- choose a valid unit/prop cell while a weapon is armed
- inspect any unit or threat

Keyboard shortcuts mirror core commands for development and accessibility, but pointer play is complete by itself. Grid legality is always revalidated by the domain transition.

### Synchronization and animation

After each accepted domain command, presentation consumes returned events in order, then synchronizes visual transforms, HP/EN displays, telegraphs, and visibility from canonical state. A unit transform never writes a new logical grid position back into combat state.

Movement, attacks, damage flashes, and knockouts use short transform/material animations. Input is locked only while the small current event queue is playing. Skipping or completing an animation produces the same final canonical state.

## Native Bevy UI

Keep the board dominant and use screen-space native Bevy UI only:

- **Top-left objective panel:** primary state, Turnabout bonus state, round, and phase.
- **Top-right threat panel:** one row per committed intent showing attacker, weapon, fixed cells, current intended occupant where present, normal damage, and hit chance. Canceled intents remain visibly crossed out until resolution ends.
- **Bottom-left unit panel:** selected mech HP, EN, Move/Action availability, and current stance.
- **Bottom command bar:** Move, the selected mech's three authored weapons, Counter, Guard, Evade, Finish Unit, and Resolve Attacks when eligible.
- **Context preview:** target name/cell, hit chance, normal/critical damage, EN cost, push/collision/hazard consequence, and affected footprint before confirmation.
- **Result overlay:** victory plus optional-objective result, or failure, with Restart.
- **Status line:** concise explanation for rejected commands or missing asset failures.

World-space readability uses color and shape together:

- cyan: selected/reachable
- amber: legal attack footprint and preview
- red striped/pulsing overlay: committed enemy footprint
- white outline: current intended occupant
- green shield, blue motion mark, orange return-fire mark: Guard, Evade, Counter

Do not rely on color alone. Telegraphs remain visible while command menus are open.

## Error Handling and Restart

- Asset failure keeps the app alive with a visible error overlay naming the missing asset and a logged diagnostic.
- Invalid player commands return `BattleError`, preserve state, and update the status line.
- Presentation entities are disposable projections. Restart despawns the Mission 1 presentation root, constructs a fresh canonical mission state, clears transient selection/event queues, and respawns from that state.
- Restart does not reuse HP, EN, objectives, intents, allowances, or reaction state from the failed run.

## Testing Strategy

Most coverage is ordinary pure Rust `cargo test` without creating a window or renderer.

Required focused tests cover:

- legal and illegal orthogonal movement, blockers, occupancy, and Manhattan range
- free activation order and Move/Action exactly once in either order
- skipping allowances when finishing an activation
- all nine weapon definitions and EN deduction exactly once
- seeded hit, miss, critical, deterministic normal damage, and knockout
- preview values matching resolution inputs
- fixed intent footprints hitting empty space, a moved-in player, or an enemy
- attacker knockout canceling an intent
- Counter range/EN behavior, Guard reduction, Evade chance, and no recursive reactions
- Push 1 movement, collision, hazard entry, and no duplicate damage
- explosive destruction and exactly-once area damage
- primary victory, Turnabout completion, failure, and clean restart
- the authored opening plan placing the Striker in the Artillery footprint

Use only a few Bevy `App` integration tests for behaviors that actually depend on scheduling or presentation handoff:

- a canonical unit move updates the linked visual transform
- fixed intents create the expected telegraph entities
- restart replaces the presentation root and clears transient interaction state

Those tests use minimal plugins and do not initialize a renderer/window. The viability checkpoint and final presentation are validated by running the real desktop application.

## CI and Validation

GitHub Actions runs:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-targets`
4. `cargo build --release`

The branch records evidence in `docs/validation/hpa-632.md` for both gates.

### In-ticket Bevy viability checkpoint

Before the remaining combat systems are considered safe to build on, the same application must demonstrate:

- Rust 2024 / Bevy 0.19 boots locally
- orthographic 3D board renders
- at least one checked-in glTF scene renders
- mesh picking selects a logical cell
- one pure `BattleState` movement drives a 3D unit transform
- native Bevy UI overlays the 3D board
- one pure domain test runs without a renderer/window

A genuine blocker revises the HPA-632 design and stack in this branch rather than creating a separate spike PR.

### Final playtest gate

Play the retained Mission 1 and record:

- whether the 2.5D battlefield/native UI workflow remained maintainable without engine-building work
- whether telegraphs stayed readable throughout a full mission
- whether the opening Artillery/Striker setup and later environment opportunities made intent manipulation regularly useful
- whether the first-clear duration stayed near 15–20 minutes
- whether restart produced clean state

HPA-635 must not begin until both viability questions are answered yes. Failed answers lead to tuning, simplification, or revision inside HPA-632.

## Explicitly Out of Scope

Campaign save/Continue, title flow, finished VN scenes, credits, upgrades, pilot skills, additional enemy archetypes, bosses, extra playable units, deployment, Spirit Commands/SP, morale, equipment, ammo, generic statuses, procedural maps, external mission authoring, rewind/checkpoints, score/rank, difficulty modes, dedicated battle-animation scenes, backend services, physics, analytics, and backward-compatibility layers.

## Delivery Rule

One ticket equals one player-visible PR. Implementation may use staged commits for the viability checkpoint, pure combat rules, presentation, and final tuning, but those stages are not separately shipped. Generalize only when a later concrete Scorpius ticket supplies a second consumer.
