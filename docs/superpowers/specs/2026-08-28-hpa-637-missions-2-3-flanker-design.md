# HPA-637 Missions 2–3 and Flanker Design

## Context

HPA-635 is complete on `main`; the baseline for this work is `ca4a281cbb72261429fe6a5247816fa25aacff62`. The campaign now has a working Title → VN → Briefing → Battle → Aftermath → Upgrade loop, persistent credits/upgrades, once-per-mission pilot skills, and an authored Mission 1 definition. `MissionId::Two` is currently only a saved handoff state and `mission_definition(MissionId::Two)` returns `None`.

HPA-637 is the next unblocked Scorpius issue. It expands that validated loop to three complete missions and introduces the fourth regular enemy, Flanker. This is a bounded architectural/content slice: two new authored missions need a small objective seam and one new deterministic enemy behavior, but they do not justify a generic mission framework, behavior tree, scripting layer, or new combat subsystem.

The delivery remains **one ticket = one PR**. The same draft PR that carries this design/plan is intended to receive the implementation.

## Goals

1. Make Missions 2 and 3 fully playable through the existing campaign/save/upgrade loop.
2. Make Mission 2 win/fail from a protect/survive condition rather than destroy-all.
3. Make Mission 3 win/fail from interception, extraction, and a clear deadline rather than destroy-all.
4. Add Flanker as a visibly and mechanically distinct fourth regular enemy with high movement/evasion, low durability, objective pressure, and one simple attack.
5. Keep objectives visible in briefing, battle HUD, and terminal results.
6. Keep optional objectives credit-only and never required for campaign advancement.
7. Preserve Mission 1's validated combat behavior, especially committed intents and its exact authored opening threats.
8. Keep the implementation small, typed, deterministic, Bevy-free in `domain`, and compatible with the current single-crate architecture.

## Non-goals

Do not add:

- a generic objective framework or callback/plugin registry;
- neutral factions, objective-unit roles, escorts, or deployment selection;
- behavior trees, utility AI, pathfinding packages, stealth, teleportation, or initiative systems;
- new playable mechs, Bulwark, Controller, bosses, mission select, branching, or difficulty modes;
- new hazard types, status effects, healing, equipment, items, or progression tracks;
- RON/JSON mission authoring, scripting, a content pipeline, or a second crate;
- new VN art generation or a new glTF pipeline;
- save migrations or backward-compatibility branches.

Existing save files containing `MissionId::One` or `MissionId::Two` continue to deserialize naturally after enum expansion; no special migration code is required.

## Approaches considered

### A. Generic objective/AI framework

Model objectives as trait objects/callbacks and introduce reusable enemy behavior policies.

**Rejected.** HPA-637 has exactly three primary objective shapes and one new enemy archetype. A framework would add indirection before a fourth consumer exists, make deterministic tests harder to read, and violate the ticket's explicit “only seams these missions consume” rule.

### B. Add a neutral protected objective unit

Create a new faction or unit role for a relay/core that enemies can attack in Mission 2.

**Rejected.** Current targeting, activation readiness, selection, combat faction checks, and HUD all assume the two existing factions. Mission 2 can teach defense more directly by protecting the already-fragile Gunner. That reuses reactions, Aegis, movement, targeting, HP, and locked telegraphs without another domain concept.

### C. Closed mission rules + existing squad target + Flanker special case

Store one closed mission rule row inside `BattleState`, generalize the optional-result bit, move the existing Mission 1 opening plan into authored data, and teach `enemy.rs` only the Flanker decisions HPA-637 needs.

**Chosen.** It is the smallest change that makes both missions honest about their objectives while removing the existing Mission-1-only opening hardcoding from the domain.

---

## Architecture

### 1. Keep objective shapes closed and explicit

Add the following plain-Rust value types in `src/domain/model.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimaryObjective {
    EliminateAllEnemies,
    ProtectThroughRound {
        target: UnitId,
        round: u16,
    },
    InterceptBeforeEscape {
        target: UnitId,
        escape: GridPos,
        deadline_round: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalObjective {
    Turnabout,
    ProtectTargetAtHalfHp {
        target: UnitId,
    },
    VictoryByRound {
        round: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnemyOpening {
    pub unit: UnitId,
    pub destination: GridPos,
    pub target: Option<UnitId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionRules {
    pub primary: PrimaryObjective,
    pub optional: OptionalObjective,
    pub opening_plan: &'static [EnemyOpening],
}
```

These are not an extensibility framework. They are a closed description of the three primary and three optional conditions that Missions 1–3 actually use, plus the authored opening data already present as hard-coded Mission 1 logic.

`BattleState::new` receives `MissionRules` directly and stores it. Expose a read-only `rules()` accessor for presentation/tests. Do not add a mission identifier to `BattleState`; the battle only needs rules, while `ActiveMission`/`MissionDefinition` remain the presentation/campaign identity boundary.

### 2. Make optional progress/result generic

Replace the Mission-1-specific field names:

```rust
pub struct ObjectiveProgress {
    pub optional_complete: bool,
}

pub struct MissionResult {
    pub victory: bool,
    pub optional_complete: bool,
    pub rounds: u16,
}
```

`CampaignState::complete_mission` awards `definition.optional_reward` only when `result.optional_complete` is true. No objective-specific reward logic belongs in campaign progression.

`BattleEvent::OptionalObjectiveCompleted` stays generic and unchanged. Presentation changes its playback copy from `TURNABOUT ACHIEVED` to `BONUS OBJECTIVE COMPLETE`.

### 3. Terminal evaluation remains one `BattleState` responsibility

`check_terminal_state` continues to be the single place that seals `MissionResult`, terminal phase, active-unit cleanup, and terminal events. It evaluates the current `MissionRules::primary` instead of always checking destroy-all.

A global squad wipe is defeat for every mission and is checked first.

#### Mission 1 — `EliminateAllEnemies`

- victory when at least one player unit is alive and no enemy unit is alive;
- defeat when no player unit is alive;
- otherwise continue.

This preserves current Mission 1 behavior exactly.

#### Mission 2 — `ProtectThroughRound { target: GUNNER, round: 3 }`

- defeat immediately if the protected Gunner is knocked out;
- victory only when `phase == BattlePhase::EnemyPlanning && battle.round() >= 3` while Gunner is alive;
- killing every enemy early does **not** end the mission;
- other player casualties do not independently fail the mission unless they produce the global squad wipe.

The phase check matters because `round` already names the active player/enemy round. It prevents a Round-3 player attack or an early intent inside Round-3 resolution from declaring victory before the full third enemy resolution has completed. `resolve_enemy_phase` returns to `EnemyPlanning` only after every committed intent has resolved, then `begin_round()` performs the terminal check before planning Round 4.

No new “enemy phases completed” counter is needed.

#### Mission 3 — `InterceptBeforeEscape { target: COURIER, escape: (8, 2), deadline_round: 4 }`

- victory immediately when Courier is knocked out, regardless of surviving escorts;
- defeat when Courier reaches `(8, 2)`;
- defeat when `phase == BattlePhase::EnemyPlanning && battle.round() >= 4` while Courier is still alive;
- killing every escort without stopping Courier does not end the mission;
- global squad wipe is still defeat.

The extraction check is position-based and deterministic. The Round-4 fallback prevents indefinitely body-blocking the exit or stalling the runner behind other units.

### 4. Optional completion is evaluated only where its condition exists

Add one private `optional_condition_met()`/`mark_optional_complete()` path in `BattleState`; do not introduce callbacks.

- `Turnabout` is event-driven exactly as today: qualifying enemy/environment damage to an enemy marks the bonus once and emits `OptionalObjectiveCompleted`.
- `ProtectTargetAtHalfHp { target }` is checked when Mission 2 reaches victory. It succeeds when `target.hp * 2 >= target.stats.max_hp`.
- `VictoryByRound { round }` is checked when Mission 3 reaches victory. It succeeds when `battle.round() <= round`.

When a terminal victory satisfies a not-yet-complete terminal bonus, emit `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat does not newly award a terminal-only bonus.

This keeps `ObjectiveProgress` to one bit and avoids per-objective state that has no consumer.

### 5. Move Mission 1 opening behavior out of domain hardcoding

`enemy.rs` currently hard-codes Mission 1 opening destinations and forced targets by archetype/position. That cannot coexist cleanly with two more authored missions.

Replace it with `MissionRules::opening_plan`:

- round 0: for each living enemy, look up its `EnemyOpening` row by `UnitId`; if present, move directly to `destination`; if absent, remain in place;
- opening intent: if that row has `target: Some(id)` and the target is alive, use the target's current cell as the forced footprint center; otherwise use normal targeting;
- later rounds continue to use deterministic `choose_enemy_destination`.

The authored opening move remains a direct scripted placement, matching existing Mission 1 semantics. It does not become a path planner and does not spend or validate an activation.

Mission 1's opening rows are:

```text
Rifleman L  -> (2,5), target Gunner
Rifleman R  -> (6,5), target Interceptor
Striker     -> (4,6), target Vanguard
Artillery   -> (4,0), target Vanguard
```

Existing Mission 1 tests must continue to pin these exact positions, intent order, and mortar footprint.

### 6. Add a small shared regular-enemy catalog

Create `src/mission/enemies.rs` because Missions 1–3 now share the same regular archetypes and weapon values.

It owns factory functions for Rifleman, Striker, Artillery, and Flanker plus their four weapon specs. Mission modules continue to own unit IDs, names, positions, board layout, opening plan, and roster composition.

Keep the existing values unchanged:

| Enemy | HP | Armor | Move | Accuracy | Evasion | Weapon |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rifleman | 9 | 1 | 2 | 72 | 5 | Service Rifle |
| Striker | 12 | 2 | 2 | 78 | 10 | Shock Claw |
| Artillery | 10 | 1 | 1 | 90 | 0 | Siege Mortar |
| **Flanker** | **8** | **0** | **4** | **82** | **30** | **Skirmish Carbine** |

Flanker's `Skirmish Carbine` is deliberately simple:

```text
range 1–2
shape Single
base damage 4
hit modifier +5
crit 10%
EN cost 0
no push
not a counter weapon
```

Do not add stealth, teleportation, initiative changes, special damage, status effects, or a second weapon.

### 7. Flanker behavior is one deterministic branch in the current planner

Add `UnitArchetype::Flanker`; keep archetype-driven behavior in `enemy.rs`.

#### Mission 2 protection pressure

When the primary rule is `ProtectThroughRound { target, .. }`, a Flanker:

1. treats the protected unit's **current position** as its goal;
2. scores reachable cells by:
   - distance to its weapon's legal range band around that goal;
   - Manhattan distance to the goal;
   - more open orthogonal neighbors first;
   - then `y`, then `x` for deterministic tie-breaking;
3. when choosing an attack footprint, prefers a legal footprint containing the protected target before the existing threatened-count/player-priority ordering.

This makes the Flanker chase a moved Gunner rather than a stale authored cell.

#### Mission 3 courier pressure

When the primary rule is `InterceptBeforeEscape` and the Flanker is the designated target, it:

1. scores reachable cells by Manhattan distance to the authored extraction cell;
2. prefers more open orthogonal neighbors on equal distance;
3. then uses `y`, then `x` as the deterministic tie-break;
4. still commits one normal Skirmish Carbine intent after movement. It does not retarget a committed intent during the player phase.

Other archetypes keep their current later-round movement and targeting. No reusable behavior-policy abstraction is added.

### 8. Mission 2 is a short three-round defense

Create `src/mission/mission_two.rs`.

#### Identity and rewards

```text
Title: Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Base reward: 400 credits
Bonus reward: 100 credits
Unlocks: Mission 3
```

Rules:

```rust
MissionRules {
    primary: PrimaryObjective::ProtectThroughRound {
        target: squad::ids::GUNNER,
        round: 3,
    },
    optional: OptionalObjective::ProtectTargetAtHalfHp {
        target: squad::ids::GUNNER,
    },
    opening_plan: &MISSION_TWO_OPENING,
}
```

#### Board and deployment

Use another compact 9×9 board so the existing camera/HUD framing stays unchanged.

```text
Player deployment
Vanguard    (3,7)
Gunner      (4,6)   <- protected target
Interceptor (5,7)

Blocking cells
(3,3), (5,3), (2,6), (6,6)

Hazards
(1,5), (7,5)

Explosive
(6,4), HP 4
```

#### Enemy roster and opening

Use one of each regular enemy so the player reads competing threat shapes without a larger wave system:

```text
Rifleman  id 21, starts (2,2), opening -> (2,4), target Gunner
Striker   id 22, starts (4,3), opening -> (4,5), target Gunner
Artillery id 23, starts (4,0), opening -> (4,0), target Gunner
Flanker   id 24, starts (8,4), opening -> (5,5), target Gunner
```

The opening positions keep each forced target legal for its weapon. Four locked threats converge on Gunner, immediately making movement plus Guard/Evade/Aegis meaningful; the player can then reduce pressure by moving the protected Gunner, knocking out threats, and using the existing environment.

There are no reinforcements or waves. If the player clears the board before Round 3, the mission still ends only after Gunner survives the required third enemy resolution.

#### VN copy

Reuse the existing checked-in `relay_nine_bg.png`, Control portraits, and Vanguard portrait. Add no new image assets.

Pre-mission, three concise lines:

1. **Control:** `Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.`
2. **Vanguard:** `Then Gunner stays standing. We move around the locks, cover the weak angles, and hold.`
3. **Control:** `New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.`

Aftermath, two lines:

1. **Vanguard:** `Uplink complete. Relay Nine can finally hand us the enemy route data.`
2. **Control:** `It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.`

### 9. Mission 3 is a focused interception race

Create `src/mission/mission_three.rs`.

#### Identity and rewards

```text
Title: Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 4.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
Base reward: 500 credits
Bonus reward: 150 credits
Unlocks: Mission 4
```

Rules:

```rust
MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 2),
        deadline_round: 4,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
}
```

#### Board and deployment

Keep 9×9 and reuse only existing terrain mechanics:

```text
Player deployment
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking cells
(4,3), (4,4), (4,5)

Hazard
(2,5)

Explosive
(6,3), HP 4

Extraction
(8,2)   <- logical objective cell only; no new prop type
```

The three-cell wall makes the courier choose an open lane around the center while preserving room for the squad to cut across, push enemies into terrain, or use the explosive.

#### Enemy roster and opening

```text
Courier   id 31, Flanker, starts (0,6), opening stays (0,6), no forced target
Rifleman  id 32, starts (3,2), opening -> (3,4), target Vanguard
Striker   id 33, starts (6,6), opening -> (5,7), target Interceptor
```

Courier's movement-4 escape preference makes it the strategic target. The two escorts create readable locked threats but do not gate victory; defeating Courier immediately wins even if they survive.

The direct path from `(0,6)` to `(8,2)` is twelve Manhattan steps before accounting for occupancy. With movement 4 and the central wall/open-lane tie-break, an unopposed Courier threatens extraction after roughly three later-round movement passes. The Round-4 deadline guarantees failure even if body-blocking delays the exact extraction cell.

#### VN copy

Reuse the same existing VN art.

Pre-mission, three lines:

1. **Control:** `Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.`
2. **Vanguard:** `We cut across and stop it. Escorts are secondary — the Courier is the mission.`
3. **Control:** `Extraction is at the east marker. If it gets out, or Round 4 closes, the data is gone.`

Aftermath, two lines:

1. **Vanguard:** `Courier down. The route keys are intact.`
2. **Control:** `Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.`

### 10. Mission definition dispatch expands only to the next handoff

`MissionId` becomes:

```rust
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
}
```

Add a small `number()` helper only because campaign handoff copy needs to print the saved next mission.

`mission_definition` returns definitions for One, Two, and Three; Four remains `None` as the HPA-523 handoff.

Do not introduce a `Vec`, registry, hashmap, plugin, or dynamic registration.

### 11. Campaign flow continues through authored missions

Keep the existing screens and state machine. Do not add a “mission select” or another screen.

Routing becomes:

```text
NEW GAME
  Mission 1 pre-story

CONTINUE
  next=One        -> PreMissionStory
  next=Two/Three  -> Upgrade
  next=Four       -> NextMission handoff

Victory
  Battle -> Aftermath -> Upgrade

Upgrade PROCEED
  if mission_definition(next_mission).is_some()
      -> PreMissionStory
  else
      -> NextMission
```

Therefore a normal live playthrough is:

```text
M1 battle -> aftermath -> upgrade -> M2 story/briefing/battle
          -> aftermath -> upgrade -> M3 story/briefing/battle
          -> aftermath -> upgrade -> M4 unlocked handoff
```

The saved state still stores only `next_mission`, credits, and upgrades. No new campaign-state field is required.

`next_mission_copy` becomes generic (`MISSION {n} UNLOCKED`) so the final HPA-637 handoff says Mission 4, not Mission 2.

### 12. Briefing, HUD, events, and results become objective-generic

`MissionDefinition` already owns human-readable primary/bonus copy; keep those fields.

`HudSnapshot::from_battle` appends rule-specific progress without putting presentation strings into `domain`:

- Eliminate: enemy count remaining.
- Protect: current round/required round plus protected unit HP.
- Intercept: current round/deadline plus Courier Manhattan distance to extraction.
- Turnabout bonus: `Complete` / `Not yet`.
- Half-HP bonus: `On track` / `Missed` based on current HP threshold.
- Victory-by-round bonus: `Available` / `Missed` based on current round; terminal state uses `optional_complete`.

Result overlay takes both `MissionResult` and `MissionDefinition` and renders:

```text
MISSION COMPLETE | MISSION FAILED
<mission title>
PRIMARY  <primary objective> · Complete/Failed
BONUS    <optional objective> · Achieved/Missed
```

Aftermath reward copy changes `Turnabout +...` to `Bonus +...`; the objective text itself was already shown in the result overlay and aftermath is only the persisted credit receipt.

`OptionalObjectiveCompleted` playback copy becomes `BONUS OBJECTIVE COMPLETE`.

### 13. Flanker visual distinction reuses the checked-in glTF

Do not modify the glTF or add an art generator for one enemy.

In `battlefield.rs`:

- map `UnitArchetype::Flanker` to the existing Interceptor scene (`scene 2`) to give it a light/fast silhouette distinct from Rifleman/Striker/Artillery;
- render Flanker at scale `0.62` instead of the normal `0.72`;
- add a persistent red/orange under-ring using the already-created `ring_mesh` and `telegraph_edge` material.

Enemy rotation already differentiates allegiance; the ring differentiates Flanker from the player Interceptor. Movement 4, evasion 30, low HP/armor, and its objective-seeking planner provide the mechanical distinction.

Rename debug entity labels such as `Mission 1 Presentation` to `Mission Presentation` where touched. Do not rename the existing `mission_one.gltf` asset or broaden asset loading in this ticket.

### 14. Shared enemy data is the only content extraction

`mission_one.rs` should consume the new `mission::enemies` factories so Rifleman/Striker/Artillery values are defined once. Do not otherwise reorganize Mission 1.

Mission-specific files own:

- `SquadDeployment`;
- board cells/props;
- stable enemy IDs/names;
- opening rows;
- mission rules;
- dialogue;
- rewards/definition.

This mirrors the existing `mission::squad` boundary and is enough for Missions 4–5 to reuse later without pre-building their architecture.

---

## Testing strategy

All automated tests remain headless.

### Domain/objective tests

Pin these behaviors directly:

- Mission 1 still wins only when all enemies are knocked out and its Turnabout bonus is event-driven.
- Mission 2 does not win when all enemies are knocked out before Round 3.
- Mission 2 immediately loses when Gunner is knocked out.
- Mission 2 wins from `EnemyPlanning` at Round 3 with Gunner alive.
- Mission 2 half-HP bonus is achieved at `hp * 2 >= max_hp` and missed below it.
- Mission 3 wins immediately when Courier is knocked out while escorts remain.
- Mission 3 does not win from escort clear alone.
- Mission 3 loses when Courier is on `(8,2)`.
- Mission 3 loses at the Round-4 `EnemyPlanning` deadline while Courier remains alive.
- Mission 3 early bonus is achieved at Round 2 and missed at Round 3.
- terminal-only bonuses emit `OptionalObjectiveCompleted` before `MissionCompleted` exactly once.

### Enemy planner tests

- Mission 1 exact authored opening positions/targets/intents remain unchanged.
- Mission 2 opening creates four Gunner-directed readable threats.
- a Mission 2 Flanker destination prioritizes the protected Gunner and open-neighbor tie-break.
- a Mission 2 Flanker intent prefers Gunner when Gunner is legally targetable.
- the Mission 3 Courier destination reduces distance to extraction and does not switch to ordinary player-chasing behavior.
- Flanker behavior remains deterministic for a fixed state; no RNG is added to movement/target selection.

### Mission authoring tests

Each new mission pins:

- 9×9 board and the listed blocking/hazard/explosive cells;
- exact player deployment;
- exact enemy roster/archetypes/positions;
- `MissionRules` target/round/escape/deadline;
- title/objective copy/rewards/unlock;
- upgrades still project through `build_player_squad` once.

### Campaign/presentation tests

- completing M1 → M2 → M3 advances `next_mission` to Four and normal completion rewards do not depend on bonuses;
- bonus only changes credits;
- `MissionId::Three/Four` save/load round-trip;
- Continue routes One → story, Two/Three → Upgrade, Four → handoff;
- Upgrade `PROCEED` routes Two/Three to story and Four to handoff;
- briefing/HUD/result copy is correct for all three objective shapes;
- battle entry/restart builds Mission 2 or Mission 3 from the active definition and current upgrades;
- Flanker is mapped to the fast silhouette and marker path without adding an asset dependency.

### Manual validation

Record `docs/validation/hpa-637.md` with at least:

1. New Game through Mission 1 completion, upgrade, Mission 2 story/briefing.
2. Mission 2 success by surviving Round 3 with enemies still alive.
3. Mission 2 failure from Gunner KO.
4. Mission 2 bonus achieved and missed examples.
5. Mission 3 Courier visibly routes toward extraction while escorts retain locked telegraphs.
6. Mission 3 victory with escorts still alive.
7. Mission 3 extraction failure and Round-4 deadline failure.
8. Mission 3 early bonus achieved/missed.
9. Save/quit/Continue before Missions 2 and 3, with upgrades retained.
10. Final Mission 4 unlocked handoff after Mission 3.
11. Full CI-equivalent command output.

The playtest gate is qualitative but specific: Missions 2–3 should remain short-session encounters and should not require repeatedly passing empty/low-pressure rounds. If authored tuning is off, change mission positions/stats/rewards within this PR; do not add a new system to compensate.

## File boundaries

Expected new files:

- `src/mission/enemies.rs` — shared regular enemy constructors/weapon specs.
- `src/mission/mission_two.rs` — all Mission 2 authored data.
- `src/mission/mission_three.rs` — all Mission 3 authored data.
- `docs/validation/hpa-637.md` — implementation/playtest evidence.

Expected modified files:

- `src/domain/model.rs` — Flanker + closed mission rule/result types.
- `src/domain/battle.rs` — objective evaluation and generic bonus state.
- `src/domain/enemy.rs` — authored opening plan + Flanker movement/targeting.
- `src/mission/mod.rs` — modules, MissionId 1–4, definition dispatch.
- `src/mission/mission_one.rs` — consume shared enemy catalog + authored opening/rules.
- `src/campaign/progression.rs` — generic optional reward bit.
- `src/presentation/battlefield.rs` — Flanker visual mapping/marker and generic debug name.
- `src/presentation/interaction.rs` — exhaustive Flanker pilot-skill rejection and generic presentation name if touched.
- `src/presentation/ui.rs` — dynamic objective progress + generic results/events.
- `src/presentation/campaign_ui.rs` — continuous M1→M2→M3 routing and generic handoff/reward copy.
- `tests/presentation_app.rs` — campaign/battle presentation integration coverage.
- `README.md` — three-mission campaign/player-facing behavior.
- `CLAUDE.md` — current architecture/content state and HPA-637 rule-of-record references.

`src/app.rs`, save/session code, and the glTF should only change if an implementation test demonstrates a concrete need; the current dispatch/session seams are already mission-generic.

## Acceptance mapping

- Continuous M1–M3 campaign: MissionId/definition dispatch + Upgrade/Continue routing.
- Mission 2 protect/survive: `ProtectThroughRound` with Gunner target; no enemy-clear victory.
- Mission 3 time pressure/interception: Courier KO success, extraction/deadline failure; no escort-clear victory.
- Flanker distinction: movement 4/evasion 30/HP 8/armor 0, protected-target or extraction movement preference, fast silhouette + under-ring.
- Objectives in briefing/battle/results: existing `MissionDefinition` copy + rule-aware HUD + definition-aware result overlay.
- Optional credits only: generic `optional_complete` consumed solely by reward calculation.
- Focused tests: objective, AI, campaign, presentation suites above.
- Short-session manual gate: explicit HPA-637 validation checklist, with tuning-only response if needed.

## Decision summary

HPA-637 should **extend the current code, not replace it**: three closed objective shapes, one generic optional-result bit, one authored opening slice, one shared regular-enemy catalog, and one Flanker branch in the deterministic planner. Mission 2 protects the existing Gunner instead of inventing an objective unit; Mission 3 treats one Flanker as Courier and wins on that target alone. Existing UI/campaign screens remain the composition surface, and existing combat mechanics remain the tactical vocabulary.
