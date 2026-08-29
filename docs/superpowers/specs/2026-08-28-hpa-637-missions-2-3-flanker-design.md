# HPA-637 Missions 2–3 and Flanker Design

## Context

HPA-635 is complete on `main`; the baseline for this work is `ca4a281cbb72261429fe6a5247816fa25aacff62`. Scorpius already has the complete Title → VN → Briefing → Battle → Aftermath → Upgrade loop, save/continue, credits/upgrades, pilot skills, and one authored Mission 1 definition. `MissionId::Two` is currently only a saved handoff.

HPA-637 is the next unblocked Scorpius issue. It expands that validated loop to three complete missions and introduces the fourth regular enemy, Flanker. Two new missions need a small objective seam and one enemy needs deterministic objective-aware behavior, but they do not justify a generic objective framework, behavior tree, scripting layer, or new combat subsystem.

The delivery remains **one ticket = one PR**. This planning PR is also the implementation PR for HPA-637.

## Goals

1. Make Missions 2 and 3 fully playable through the existing campaign/save/upgrade loop.
2. Make Mission 2 a defense encounter where Gunner survival is the primary concern without forcing empty-board busywork.
3. Make Mission 3 a real Courier chase where extraction is reachable in normal play and a later deadline is only the anti-stall backstop.
4. Add Flanker as a visibly and mechanically distinct fourth regular enemy: fast, evasive, fragile, objective-seeking, one attack.
5. Show primary + bonus objectives in briefing, live HUD, and terminal results.
6. Keep bonuses credit-only and never required for progression.
7. Preserve Mission 1's exact validated opening threats and committed-intent semantics.
8. Keep everything typed, deterministic, single-crate, and Bevy-free in `domain`.

## Non-goals

Do not add a neutral faction/objective unit, objective callback/trait registry, behavior tree/utility-AI framework, pathfinding package, stealth, teleportation, a new initiative system, new playable units, deployment selection, new hazard types, status effects, bosses, mission select, branching, difficulty, RON/JSON mission authoring, another crate, a runtime asset pipeline, new VN art, or save migration/backward-compatibility code.

The existing checked-in `assets/models/mission_one.gltf` may be extended with one additional authored scene. That is content authoring inside the current asset and does not introduce a pipeline or new dependency.

Existing saves containing Mission 1/2 enum values deserialize naturally after adding later enum variants; no migration branch is needed.

## Chosen shape

Use one closed `MissionRules` row inside `BattleState`, move the already-hard-coded Mission 1 opening plan into authored data, share fixed regular-enemy constructors under `mission`, and add one explicit Flanker branch to the current deterministic enemy planner.

This keeps the architecture at the same level as HPA-635: authored differences live in mission data; battle rules stay plain Rust; presentation/campaign remain composition.

---

## 1. Closed mission rules in the plain-Rust domain

Add in `src/domain/model.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimaryObjective {
    EliminateAllEnemies,
    ProtectThroughRound { target: UnitId, round: u16 },
    InterceptBeforeEscape {
        target: UnitId,
        escape: GridPos,
        deadline_round: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalObjective {
    Turnabout,
    ProtectTargetAtHalfHp { target: UnitId },
    VictoryByRound { round: u16 },
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

`BattleState::new` receives/stores `MissionRules`; `BattleState::rules()` exposes a copy for presentation/tests. Do not put `MissionId` into domain state.

These enums are the three authored shapes Missions 1–3 need, not an extensibility framework.

## 2. Make bonus progress/result generic

Replace the Mission-1-specific field names with:

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

Campaign reward code checks only `result.optional_complete`. `BattleEvent::OptionalObjectiveCompleted` remains unchanged; presentation calls it `BONUS OBJECTIVE COMPLETE` rather than Turnabout.

The Turnabout trigger remains special inside the existing damage-observation seam. Do not move damage-source predicates into a generic objective engine.

## 3. Name the enemy-round boundary once

The protect duration and interception deadline both depend on the same subtle `begin_round()` timing: while `phase == EnemyPlanning`, `round` is the number of player/enemy rounds that have already completed; `begin_round()` checks terminal state before later movement and increments `round` only after movement/intent planning.

Keep that coupling in one helper on `BattleState`:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

Both objective arms use this helper. Do not retype the phase/round predicate in multiple match arms.

This is a naming seam for existing lifecycle semantics, not a new clock/counter.

## 4. Terminal semantics stay in `BattleState`

`check_terminal_state` remains the only place that seals result/phase and clears terminal transient state. It evaluates `MissionRules::primary`.

Global rule: if no player unit is alive, defeat.

### Mission 1: eliminate all

`EliminateAllEnemies` preserves current behavior: victory when no enemy remains and at least one player is alive.

### Mission 2: protect Gunner through Round 3, with immediate safety on enemy clear

`ProtectThroughRound { target: GUNNER, round: 3 }` means:

- Gunner KO → immediate defeat.
- If no living enemy remains while Gunner is alive → immediate victory; nothing remains that can interrupt the upload.
- Otherwise victory when `completed_enemy_round(3)` is true with Gunner alive.
- Other player losses are allowed unless they cause the global squad wipe.

This keeps the normal authored defense clock while avoiding multiple rounds of selecting three mechs, choosing reactions, and resolving an empty battlefield after the player has already removed every threat.

The briefing/HUD must make the alternative explicit rather than hiding a destroy-all path:

> Protect Gunner through the end of Round 3, or eliminate all attackers.

### Mission 3: intercept Courier

`InterceptBeforeEscape { target: COURIER, escape: (8,0), deadline_round: 5 }` means:

- Courier KO → immediate victory even with escorts alive.
- Courier at `(8,0)` → immediate defeat.
- Courier alive when `completed_enemy_round(5)` becomes true → deadline defeat.
- Killing escorts without Courier does not win.

Extraction is the normal failure on an open route. Round 5 is only the anti-stall backstop if body-blocking or displacement prevents exact extraction.

## 5. Mission 3 clock and geometry are binding

`begin_round()` performs its terminal check before later enemy movement, then moves enemies, checks terminal state again, commits intents, increments `round`, and enters Player phase.

Courier starts at `(0,6)`, extraction is `(8,0)`, and Flanker movement is 4. On the open 9×9 board, Manhattan distance is **14**.

```text
begin_round round 0: opening; Courier stays (0,6) -> player Round 1
begin_round round 1: later move #1, <=4 steps         -> player Round 2
begin_round round 2: later move #2, <=8 total         -> player Round 3
begin_round round 3: later move #3, <=12 total        -> player Round 4; cannot extract yet
begin_round round 4: 4 < deadline 5; later move #4 can reach (8,0)
                     -> post-movement terminal check produces extraction defeat
begin_round round 5: only reachable if extraction was blocked/delayed;
                     deadline fires before another later move
```

The important automated contracts are:

1. player Round 4 exists after three later Courier moves;
2. Courier is not on extraction at that point;
3. with an open route, resolving player Round 4 lets move #4 reach extraction and fail the mission;
4. if extraction is occupied/body-blocked, the mission may reach player Round 5, then the deadline fails before another Courier move.

Do not pin an exact “two cells remaining” intermediate coordinate; that is a tie-break detail rather than the product contract.

Player displacement can also trigger extraction early. `resolve_push` already calls `check_terminal_state`, so pushing Courier onto `(8,0)` must be covered explicitly as an immediate player-caused defeat.

## 6. Bonus completion stays one bit

- `Turnabout`: mark once on the existing qualifying enemy/environment damage event.
- `ProtectTargetAtHalfHp`: on Mission 2 victory, succeed if `target.hp * 2 >= target.stats.max_hp`.
- `VictoryByRound { round: 2 }`: on Mission 3 victory, succeed if `battle.round() <= 2`.

When a terminal victory satisfies a not-yet-complete terminal bonus, emit `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat does not newly award a terminal-only bonus.

## 7. Mission-authored opening data replaces Mission-1 hardcoding

`enemy.rs` currently knows Mission 1 positions/targets by archetype and x-position. Replace that with `MissionRules::opening_plan`:

- on round 0, look up the enemy row by `UnitId`, move directly to its authored destination, or stay if absent;
- when committing round-0 intent, use the row's living target unit position as forced center when present;
- later rounds continue to use deterministic `choose_enemy_destination`.

Authored opening placement remains direct scripted movement, matching current Mission 1 semantics; it does not become pathfinding or an activation.

Mission 1 opening remains exactly:

```text
Rifleman L  -> (2,5), target Gunner
Rifleman R  -> (6,5), target Interceptor
Striker     -> (4,6), target Vanguard
Artillery   -> (4,0), target Vanguard
```

Existing tests continue to pin positions, attacker order, intended occupants, and mortar footprint.

## 8. Shared regular-enemy catalog

Create `src/mission/enemies.rs`, mirroring the existing `squad.rs` boundary. It owns fixed constructors and weapon specs; missions still own IDs, names, board, deployment, roster, opening, rules, dialogue, and rewards.

| Enemy | HP | Armor | Move | Accuracy | Evasion | Weapon |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rifleman | 9 | 1 | 2 | 72 | 5 | Service Rifle |
| Striker | 12 | 2 | 2 | 78 | 10 | Shock Claw |
| Artillery | 10 | 1 | 1 | 90 | 0 | Siege Mortar |
| **Flanker** | **8** | **0** | **4** | **82** | **30** | **Skirmish Carbine** |

Flanker has exactly one new weapon:

```text
Skirmish Carbine
range 1–2
Single
base damage 4
hit modifier +5
crit 10%
EN cost 0
no push
not a counter weapon
```

## 9. Flanker behavior stays explicit in `enemy.rs`

Add `UnitArchetype::Flanker`.

### Mission 2 protected-target pressure

For `ProtectThroughRound`, Flanker uses the protected unit's current position as goal. Score reachable cells by:

1. distance to the Skirmish Carbine legal range band around the goal;
2. Manhattan distance to the goal;
3. more open orthogonal neighbors first;
4. `y`, then `x`.

When choosing a legal attack footprint, a Flanker prefers one containing the protected target before the existing threatened-count/player-priority ordering.

### Mission 3 Courier movement

If the Flanker is the designated `InterceptBeforeEscape` target, score reachable cells by:

1. Manhattan distance to extraction;
2. more open orthogonal neighbors first;
3. `y`, then `x`.

It still commits a normal Skirmish Carbine intent after moving. No RNG is added to movement/target selection, and committed intent never retargets during the player phase.

### Non-objective Flanker fallback

A Flanker not covered by either special objective branch must **not** stand still. Reuse the same attack-band destination scoring as Rifleman/Striker: minimize distance to legal weapon range, then nearest-player distance, then deterministic coordinates.

Extract one small local `choose_attack_band_destination(...)` helper used by Rifleman, Striker, and fallback Flanker. Do not create policy objects or a behavior abstraction. Artillery keeps its current branch.

### Existing initiative ordering is simplified, not generalized

The current `initiative(unit)` still contains the third Mission-1 positional hack: Rifleman initiative differs by `unit.position.x < 4`, while unknown archetypes resolve at 0.

Remove that positional dependency while touching the archetype match:

```rust
fn initiative(unit: &UnitState) -> i16 {
    match unit.archetype {
        UnitArchetype::Striker => 30,
        UnitArchetype::Flanker => 25,
        UnitArchetype::Rifleman => 20,
        UnitArchetype::Artillery => 10,
        _ => 0,
    }
}
```

Mission 1 intent order remains Striker → Rifleman L → Rifleman R → Artillery because equal Rifleman initiative falls through to the existing attacker-ID tie-break. No initiative field, system, or authored policy data is added.

## 10. Mission 2 — Hold Relay Nine

Create `src/mission/mission_two.rs`.

### Definition

```text
Title: Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Base reward: 400
Bonus reward: 100
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

### Board and deployment

Board size is **9×9**.

```text
Player deployment
Vanguard    (3,7)
Gunner      (4,6)  protected
Interceptor (5,7)

Blocking:  (3,3), (5,3), (2,6), (6,6)
Hazards:   (1,5), (7,5)
Explosive: (6,4), HP 4
```

### Enemy roster/opening

```text
Rifleman  id 21, starts (2,2), opening -> (2,4), target Vanguard
Striker   id 22, starts (4,3), opening -> (4,5), target Gunner
Artillery id 23, starts (4,0), opening -> (4,0), target Gunner
Flanker   id 24, starts (8,4), opening -> (5,5), target Interceptor
```

This deliberately creates competing threats instead of four attacks on Gunner. Reactions matter across the squad; two high-pressure locks still threaten Gunner. From later rounds onward, Flanker chases the protected Gunner.

There are no reinforcements or waves. Clearing every attacker wins immediately because the upload can no longer be interrupted; otherwise Gunner must survive through the real Round-3 boundary.

### VN copy

Reuse only existing `relay_nine_bg.png`, Control portraits, and Vanguard portrait.

Pre-mission:

1. Control: `Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.`
2. Vanguard: `Then Gunner stays standing. We hold until the upload finishes — or wipe out everything that can interrupt it.`
3. Control: `New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.`

Aftermath:

1. Vanguard: `Uplink complete. Relay Nine can finally hand us the enemy route data.`
2. Control: `It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.`

## 11. Mission 3 — Cut the Courier

Create `src/mission/mission_three.rs`.

### Definition

```text
Title: Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
Base reward: 500
Bonus reward: 150
Unlocks: Mission 4
```

Rules:

```rust
MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 0),
        deadline_round: 5,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
}
```

### Board and deployment

Board size is **9×9**.

```text
Player deployment
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking:   (4,3), (4,4), (4,5)
Hazard:     (2,5)
Explosive:  (6,3), HP 4
Extraction: (8,0)
```

The extraction cell is a logical domain objective, not a new prop type. Presentation renders a persistent white objective ring on `(8,0)` using the existing `ring_mesh` + `intended_target` material.

### Enemy roster/opening

```text
Courier   id 31, Flanker, starts/stays (0,6), no forced target
Rifleman  id 32, starts (3,2), opening -> (3,4), target Vanguard
Striker   id 33, starts (6,6), opening -> (5,7), target Interceptor
```

Courier is the strategic target; escorts create readable locked threats but never gate victory. With an open route the Courier threatens extraction on its fourth later movement, after the player has received Round 4. If the exit is blocked, Round 5 is the deadline backstop.

### VN copy

Pre-mission:

1. Control: `Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.`
2. Vanguard: `We cut across and stop it. Escorts are secondary — the Courier is the mission.`
3. Control: `Extraction is at the east marker. If it gets out, or Round 5 closes, the data is gone.`

Aftermath:

1. Vanguard: `Courier down. The route keys are intact.`
2. Control: `Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.`

## 12. Authored references are validated by mission tests

Keep runtime authored-data assumptions as direct `expect(...)` calls; do not add a validation framework. Instead, each mission module has focused authoring tests that fail during development/CI if constants are inconsistent.

Mission 2 tests assert:

- board is 9×9;
- protected Gunner exists and is a living player in the fresh battle;
- every `EnemyOpening.unit` exists and is an enemy;
- every `EnemyOpening.target` that is `Some` exists and is a player;
- every opening destination is in bounds and non-blocking.

Mission 3 tests assert all of the above plus:

- Courier exists and is `UnitArchetype::Flanker`;
- extraction `(8,0)` is in bounds, non-blocking, not a live explosive, and not a hazard;
- start `(0,6)` to extraction is Manhattan 14.

These tests catch authoring mistakes before the runtime `.expect` paths are player-reachable.

## 13. Mission dispatch grows once

Add the handoff variants together when Mission 2 is introduced:

```rust
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
}
```

Task 3 makes `mission_definition(Two)` authored while Three/Four remain `None`; Task 4 makes Three authored. Do not churn the enum and campaign match arms twice.

Final `mission_definition`: One/Two/Three `Some`, Four `None`. A small `number()` helper returns 1–4 for handoff copy.

## 14. Continuous campaign routing

Keep the existing `GameScreen` set.

```text
NEW GAME
  -> Mission 1 PreMissionStory

CONTINUE
  One       -> PreMissionStory
  Two/Three -> Upgrade
  Four      -> NextMission handoff

Victory
  -> Aftermath -> Upgrade

Upgrade PROCEED
  mission_definition(next_mission).is_some() -> PreMissionStory
  otherwise                                  -> NextMission
```

Normal live flow:

```text
M1 -> aftermath -> upgrade -> M2 story/briefing/battle
   -> aftermath -> upgrade -> M3 story/briefing/battle
   -> aftermath -> upgrade -> M4 unlocked handoff
```

Save state remains exactly next mission + credits + squad upgrades.

`next_mission_copy` becomes generic `MISSION {n} UNLOCKED` instead of hard-coded Mission 2.

## 15. Objective-generic HUD/results/rewards

Keep `MissionDefinition.primary_objective` / `optional_objective` as authored human copy. Presentation appends progress from `BattleState::rules()`:

- Eliminate: enemy count remaining.
- Protect: current/required round + protected unit HP; if no enemies remain, terminal result immediately resolves.
- Intercept: current/deadline round + Courier Manhattan distance to extraction.
- Turnabout: Complete/Not yet.
- Half-HP: On track/Missed.
- Victory-by-round: Available/Missed; terminal state uses `optional_complete`.

Mission 3 HUD shows `Round n/5`, not `n/4`.

Result overlay accepts `MissionResult + MissionDefinition`:

```text
MISSION COMPLETE | MISSION FAILED
<mission title>
PRIMARY  <primary text> · Complete/Failed
BONUS    <bonus text> · Achieved/Missed
```

Aftermath reward copy uses `Bonus +...`, not `Turnabout +...`.

## 16. Give Flanker its own checked-in glTF scene

Do not reuse the player's cyan Interceptor scene and compensate with runtime scale/ring code. Extend the existing hand-authored `assets/models/mission_one.gltf` with scene index **10**.

The file already reuses the same POSITION/NORMAL/index accessors for every simple cuboid mesh. Add:

- scene `Flanker` with nodes `49..55`;
- mesh index `10` using the existing accessors and material `10`;
- material `Flanker Magenta`:

```json
{
  "name": "Flanker Magenta",
  "pbrMetallicRoughness": {
    "baseColorFactor": [0.78, 0.08, 0.46, 1.0],
    "metallicFactor": 0.25,
    "roughnessFactor": 0.62
  },
  "emissiveFactor": [0.08, 0.0, 0.04]
}
```

Author seven cuboid nodes using mesh 10:

```text
49 Left Leg      translation (-0.16, 0.18,  0.00) scale (0.12, 0.36, 0.16)
50 Right Leg     translation ( 0.16, 0.18,  0.00) scale (0.12, 0.36, 0.16)
51 Torso         translation ( 0.00, 0.62,  0.00) scale (0.36, 0.42, 0.28)
52 Head          translation ( 0.00, 0.95,  0.00) scale (0.20, 0.20, 0.20)
53 Left Fin      translation (-0.42, 0.67, -0.10) scale (0.42, 0.08, 0.28)
54 Right Fin     translation ( 0.42, 0.67, -0.10) scale (0.42, 0.08, 0.28)
55 Rear Thruster translation ( 0.00, 0.52, -0.34) scale (0.20, 0.16, 0.34)
```

Then:

- bump `MISSION_ONE_SCENE_COUNT` from 10 to 11;
- `scene_index(UnitArchetype::Flanker) = 10`;
- keep the existing root unit scale `0.72` for every mech;
- do **not** add `unit_scale`;
- do **not** add a Flanker child under-ring or inverse-scale math.

The Flanker's magenta material + slimmer authored node proportions distinguish it visually while keeping presentation code simpler than reusing the friendly Interceptor.

Mission 3 extraction still uses the existing white `ring_mesh` + `intended_target` material under `PresentationRoot`.

Rename touched debug root text `Mission 1 Presentation` → `Mission Presentation`.

## 17. Rewards/progression tuning

Base rewards alone are 300 + 400 + 500 = **1200 credits** through Mission 3. Optional rewards are 100 + 100 + 150. Normal progression can buy useful 200/400-level upgrades without requiring bonuses or grinding.

No new progression system is needed.

---

## Testing strategy

### Domain/objective fixtures

Keep focused private outcome tests for each match arm:

- Mission 1 eliminate-all and Turnabout unchanged.
- Protect fails on target KO, wins when no enemies remain, and wins at the required `completed_enemy_round(3)` boundary with enemies still present.
- Intercept wins on target KO, ignores escort clear, fails on exact extraction and `completed_enemy_round(5)` deadline.
- Half-HP and victory-by-round bonus boundaries.
- Terminal bonus event precedes `MissionCompleted` once.
- `completed_enemy_round` is false in Player phase even when `round` has the same numeric value.

### Lifecycle tests — required before playtest

Drive the actual round machine with public battle operations.

**Mission 2:**

1. `mission_two(seed)` + `begin_round()`.
2. KO all enemies during Round 1 and assert immediate victory with Gunner alive.
3. Separately run the mission with at least one enemy alive through real player/reaction/resolve cycles.
4. Assert Gunner surviving through the third enemy resolution produces victory at the real round boundary.

No empty-board wait/round-cycling behavior is part of acceptance.

**Mission 3 open route:**

1. `mission_three(seed)` + `begin_round()` → player Round 1, Courier `(0,6)`.
2. Remove escorts only so Courier movement is isolated.
3. Resolve Rounds 1–3 → assert player Round 4 exists, Courier is not `(8,0)`, result is `None`.
4. Resolve player Round 4 → assert Courier reaches `(8,0)` during later movement #4 and extraction defeat occurs.

**Mission 3 blocked-exit backstop:**

1. Put a living player on `(8,0)` with the existing test seam so Courier cannot occupy extraction.
2. Drive through player Round 4; the next planning pass may move Courier adjacent but must not extract.
3. Assert player Round 5 is reachable with result `None`.
4. Resolve Round 5; `completed_enemy_round(5)` must fail the mission before another Courier move.

**Player-caused extraction:**

Place Vanguard/Courier aligned at `(6,0)`/`(7,0)`, call `resolve_push(VANGUARD, COURIER)`, and assert Courier moves to `(8,0)` and the mission fails immediately.

These tests lock the clock/geometry/product premise rather than leaving it to manual tuning.

### Enemy planner

- Mission 1 exact authored opening regression remains.
- Mission 2 opening intended occupants are Vanguard/Gunner/Gunner/Interceptor.
- Later Mission 2 Flanker movement/intent prioritizes protected Gunner and uses open-neighbor tie-break.
- Mission 3 Courier destination reduces extraction distance rather than chasing a normal player target.
- A non-objective Flanker uses the Rifleman/Striker attack-band fallback instead of staying still.
- Initiative order is Striker 30, Flanker 25, Rifleman 20, Artillery 10; Mission 1 left/right Rifleman order remains stable via attacker ID.
- No RNG is introduced to destination/target ordering.

### Mission authoring

Pin exact 9×9 board, deployment, roster, rules, rewards/copy/unlock for Missions 2/3. Assert rule/opening references exist and extraction is legal. Prove current upgrades still project through `build_player_squad` once.

### Campaign/presentation

- M1 → M2 → M3 normal completion advances to Four with 1200 base credits.
- Bonus changes credits only.
- save/load round-trips MissionId Four + upgrades.
- Continue routes One/story, Two/Three/Upgrade, Four/handoff.
- Upgrade Proceed routes authored Two/Three to story and Four to handoff.
- battle entry/restart uses current definition builder for M2/M3.
- briefing/HUD/result show both objective texts and dynamic progress.
- Flanker loads glTF scene 10; `MISSION_ONE_SCENE_COUNT == 11`.
- extraction marker uses `(8,0)`.

## Manual validation gate

Record `docs/validation/hpa-637.md` with:

1. continuous M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff;
2. M2 opening competing threats visible;
3. M2 enemy-clear immediately wins with Gunner alive; surviving Round 3 with enemies present also wins; Gunner KO fails;
4. M2 bonus achieved/missed;
5. M3 authored magenta Flanker/Courier and extraction ring at `(8,0)` visible;
6. M3 player Round 4 visibly exists on the authored path;
7. open-route Courier extraction after Round 4 causes defeat;
8. blocked exit reaches the Round-5 deadline backstop;
9. M3 wins with escorts alive and escort clear alone does not win;
10. M3 early bonus achieved/missed;
11. save/quit/Continue and upgrades retained before M2 and M3;
12. full CI-equivalent commands pass.

Playtest may tune encounter feel, but it must not redefine the clock/extraction semantics above.

## Expected file boundaries

New:
- `src/mission/enemies.rs`
- `src/mission/mission_two.rs`
- `src/mission/mission_three.rs`
- `docs/validation/hpa-637.md`

Modified:
- `src/domain/model.rs`
- `src/domain/battle.rs`
- `src/domain/enemy.rs`
- `src/mission/mod.rs`
- `src/mission/mission_one.rs`
- `src/campaign/progression.rs`
- `src/presentation/assets.rs`
- `src/presentation/battlefield.rs`
- `src/presentation/interaction.rs`
- `src/presentation/ui.rs`
- `src/presentation/campaign_ui.rs`
- `assets/models/mission_one.gltf`
- `tests/campaign_flow.rs`
- `tests/campaign_persistence.rs`
- `tests/presentation_app.rs`
- `README.md`
- `CLAUDE.md`

`src/presentation/sync.rs`, `src/app.rs`, save/session implementation, VN assets, Cargo files should remain unchanged unless a concrete failing integration test proves otherwise.

## Decision summary

Extend what already works: three closed primary objective shapes, three closed bonus shapes backed by one bit, one authored opening slice, one shared regular-enemy catalog, and one explicit Flanker planner branch. Mission 2 protects Gunner but does not force empty rounds after every threat is gone. Mission 3 keeps `(0,6)` → `(8,0)` so Round 4 is real, uses deadline Round 5 so normal extraction is reachable, and reserves the deadline for stalls/body-blocks. The Flanker gets a real checked-in glTF scene instead of borrowing the friendly Interceptor plus runtime compensation. Round-boundary semantics are named once, authored references are pinned in tests, and existing initiative ordering is simplified without adding a new system.