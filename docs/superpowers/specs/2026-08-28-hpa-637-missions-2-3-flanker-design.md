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

## 2. Generic optional progress/result

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

Campaign reward code checks only `result.optional_complete`. `BattleEvent::OptionalObjectiveCompleted` stays generic. Turnabout's damage-source predicate remains in the existing damage-observation seam.

## 3. Name the enemy-round boundary once

The protect duration and interception deadline both depend on `begin_round()` timing. Keep that coupling in one helper:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

While `EnemyPlanning` is active before `begin_round` increments, `round` equals the number of completed player/enemy rounds. Both objective arms use this helper; do not duplicate the predicate.

## 4. Terminal semantics

Global squad wipe remains defeat.

### Mission 1

`EliminateAllEnemies` preserves current behavior.

### Mission 2

`ProtectThroughRound { target: GUNNER, round: 3 }`:

- Gunner KO → defeat.
- No living enemies with Gunner alive → immediate victory.
- Otherwise `completed_enemy_round(3)` with Gunner alive → victory.

This removes meaningless empty-board input while retaining the real survive-three-round path. Player-facing copy says:

> Protect Gunner through the end of Round 3, or eliminate all attackers.

### Mission 3

`InterceptBeforeEscape { target: COURIER, escape: (8,0), deadline_round: 5 }`:

- Courier KO → victory even with escorts alive.
- Courier at `(8,0)` → defeat.
- `completed_enemy_round(5)` with living Courier → deadline defeat.
- Escort clear alone does not win.

Extraction is the normal failure; Round 5 is only the anti-stall backstop.

## 5. Mission 3 clock and geometry

Mission 3 is 9×9. Courier starts `(0,6)`, extraction is `(8,0)`, movement is 4, open-path Manhattan distance is 14.

```text
round 0 begin_round: opening, Courier stays           -> player Round 1
round 1 begin_round: move #1 <=4                      -> player Round 2
round 2 begin_round: move #2 <=8 total                -> player Round 3
round 3 begin_round: move #3 <=12 total               -> player Round 4; cannot extract
round 4 begin_round: 4 < deadline 5; move #4 can reach extraction -> extraction defeat
round 5 begin_round: only if blocked/delayed; deadline fires before another move
```

Automated tests must prove:

- player Round 4 exists;
- Courier is not at extraction before Round 4;
- open-route move #4 reaches extraction after player Round 4;
- blocked extraction can reach player Round 5 and then deadline-fail before another move;
- pushing Courier onto `(8,0)` via `resolve_push` fails immediately.

Do not pin an exact intermediate distance such as `== 2`; that is tie-break detail.

## 6. Bonus completion

- Turnabout: existing qualifying damage trigger.
- Hold Fast: protected Gunner at or above half HP when Mission 2 wins.
- Swift Intercept: Courier defeated by end of Round 2.

A terminal victory that newly earns a bonus emits `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat never newly earns terminal-only bonus credit.

## 7. Mission-authored openings

Replace Mission-1-specific opening movement/targeting matches with `MissionRules::opening_plan` keyed by `UnitId`.

Mission 1 remains exactly:

```text
Rifleman L -> (2,5), target Gunner
Rifleman R -> (6,5), target Interceptor
Striker    -> (4,6), target Vanguard
Artillery  -> (4,0), target Vanguard
```

Opening placement stays direct scripted movement, not pathfinding.

## 8. Shared regular-enemy catalog

Create `src/mission/enemies.rs` mirroring `squad.rs`.

| Enemy | HP | Armor | Move | Acc | Eva | Weapon |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rifleman | 9 | 1 | 2 | 72 | 5 | Service Rifle |
| Striker | 12 | 2 | 2 | 78 | 10 | Shock Claw |
| Artillery | 10 | 1 | 1 | 90 | 0 | Siege Mortar |
| Flanker | 8 | 0 | 4 | 82 | 30 | Skirmish Carbine |

Skirmish Carbine: range 1–2, Single, damage 4, hit +5, crit 10, EN0, no push, not counter-capable.

Missions keep IDs, names, positions, board, deployment, opening, rules, dialogue, and rewards.

## 9. Flanker planner and initiative

### Protect pressure

For `ProtectThroughRound`, Flanker scores reachable cells by:

1. distance to weapon range band around protected unit;
2. Manhattan distance to protected unit;
3. more open orthogonal neighbors first;
4. y, then x.

Flanker target selection prefers a legal footprint containing the protected unit.

### Courier movement

If Flanker is the `InterceptBeforeEscape` target, score by Manhattan distance to extraction, then more open neighbors, y, x.

### Non-objective fallback

A Flanker outside those special cases reuses the current Rifleman/Striker attack-band destination scoring instead of standing still. Extract one local helper; do not create policy objects.

### Initiative cleanup

Remove the remaining `unit.position.x < 4` Rifleman hack:

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

Mission 1 order stays Striker → Rifleman L → Rifleman R → Artillery via attacker-ID tie-break for equal Riflemen. No initiative field/system is added.

## 10. Mission 2 — Hold Relay Nine

Board size: **9×9**.

```text
Players: Vanguard (3,7), Gunner (4,6), Interceptor (5,7)
Blocking: (3,3), (5,3), (2,6), (6,6)
Hazards: (1,5), (7,5)
Explosive: (6,4), HP4

Rifleman 21: start (2,2), opening (2,4) -> Vanguard
Striker 22: start (4,3), opening (4,5) -> Gunner
Artillery 23: start (4,0), opening (4,0) -> Gunner
Flanker 24: start (8,4), opening (5,5) -> Interceptor
```

Definition:

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Reward: 400 + 100
Unlock: Mission 3
```

VN:

```text
Control: Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.
Vanguard: Then Gunner stays standing. We hold until the upload finishes — or wipe out everything that can interrupt it.
Control: New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.

Vanguard: Uplink complete. Relay Nine can finally hand us the enemy route data.
Control: It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.
```

## 11. Mission 3 — Cut the Courier

Board size: **9×9**.

```text
Players: Vanguard (4,7), Gunner (3,8), Interceptor (5,8)
Blocking: (4,3), (4,4), (4,5)
Hazard: (2,5)
Explosive: (6,3), HP4
Extraction: (8,0)

Courier 31 (Flanker): start/stay (0,6)
Rifleman 32: start (3,2), opening (3,4) -> Vanguard
Striker 33: start (6,6), opening (5,7) -> Interceptor
```

Definition:

```text
Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
Reward: 500 + 150
Unlock: Mission 4
```

VN:

```text
Control: Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.
Vanguard: We cut across and stop it. Escorts are secondary — the Courier is the mission.
Control: Extraction is at the east marker. If it gets out, or Round 5 closes, the data is gone.

Vanguard: Courier down. The route keys are intact.
Control: Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.
```

## 12. Authoring validation stays in tests

Do not add runtime validation machinery. Mission tests pin authored invariants.

Mission 2:

- board 9×9;
- protected Gunner exists as player;
- opening enemy/target IDs exist with correct factions;
- opening destinations are in bounds/non-blocking.

Mission 3 adds:

- Courier exists and is Flanker;
- extraction `(8,0)` is in bounds, non-blocking, non-hazard, no live explosive;
- `(0,6)` → `(8,0)` Manhattan distance is 14.

Runtime `.expect` remains acceptable because CI pins these authored constants.

## 13. Mission IDs grow once

When Mission 2 lands, add:

```rust
pub enum MissionId { One, Two, Three, Four }
```

Task 3 authors Two; Three/Four are handoffs. Task 4 authors Three. Final `mission_definition`: One/Two/Three `Some`, Four `None`. `number()` returns 1–4.

## 14. Campaign routing remains composition

```text
Continue One -> PreMissionStory
Continue Two/Three -> Upgrade
Continue Four -> NextMission

Upgrade Proceed:
  authored next mission -> PreMissionStory
  otherwise -> NextMission
```

Normal flow: M1 → Upgrade → M2 → Upgrade → M3 → M4 unlocked. Save shape remains next mission + credits + upgrades.

## 15. HUD/results/rewards become objective-generic

- Eliminate: enemy count.
- Protect: Round n/3 + Gunner HP.
- Intercept: Round n/5 + Courier Manhattan distance to extraction.
- Bonus status is objective-specific but result uses generic achieved/missed.
- `OptionalObjectiveCompleted` playback becomes `BONUS OBJECTIVE COMPLETE`.
- Aftermath uses `Bonus +...` instead of `Turnabout +...`.

No authored copy moves into domain.

## 16. Give Flanker its own checked-in glTF scene

Extend `assets/models/mission_one.gltf` with scene index 10 rather than reusing friendly Interceptor scene 2.

Add scene `Flanker` nodes 49–55, all using mesh 10. Mesh 10 reuses accessors POSITION0/NORMAL1/indices2 and material10.

Material:

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

Nodes:

```text
49 Left Leg      [-0.16,0.18, 0.00] scale [0.12,0.36,0.16]
50 Right Leg     [ 0.16,0.18, 0.00] scale [0.12,0.36,0.16]
51 Torso         [ 0.00,0.62, 0.00] scale [0.36,0.42,0.28]
52 Head          [ 0.00,0.95, 0.00] scale [0.20,0.20,0.20]
53 Left Fin      [-0.42,0.67,-0.10] scale [0.42,0.08,0.28]
54 Right Fin     [ 0.42,0.67,-0.10] scale [0.42,0.08,0.28]
55 Rear Thruster [ 0.00,0.52,-0.34] scale [0.20,0.16,0.34]
```

Then `MISSION_ONE_SCENE_COUNT = 11` and `scene_index(Flanker) = 10`.

Keep normal root scale 0.72. Do not add `unit_scale`, child under-ring, or inverse-scale math.

Mission 3 extraction still uses existing white `ring_mesh + intended_target` material.

## 17. Rewards

Base rewards: 300 + 400 + 500 = **1200 credits** through Mission 3. Optional rewards: 100 + 100 + 150. No grinding or bonus is required for useful upgrades.

---

## Required automated coverage

### Objective rules

- Mission 1 unchanged.
- Protect: target KO fail, all-enemy-clear win, Round3-boundary win with enemies alive.
- Intercept: target KO win with escort alive, escort clear non-win, extraction fail, Round5 deadline fail.
- `completed_enemy_round` false in Player phase even at same numeric round.
- bonus boundaries + event ordering.

### Mission 2 lifecycle

- Round1 enemy clear immediately wins.
- Separate run with enemies alive reaches Round2, Round3, then wins after third enemy resolution.
- Gunner KO fails.

### Mission 3 lifecycle

- Three later moves leave player Round4 playable and Courier not extracted.
- Fourth later move after player Round4 reaches `(8,0)` and fails by extraction.
- Occupied exit reaches player Round5; deadline then fails before another Courier move.
- `resolve_push` onto `(8,0)` fails immediately.

### Planner

- exact Mission1 opening regression;
- Mission2 opening occupants Vanguard/Gunner/Gunner/Interceptor;
- protect Flanker target pressure;
- Courier reduces extraction distance;
- fallback Flanker uses attack-band movement;
- initiative constants Striker30/Flanker25/Rifleman20/Artillery10;
- no RNG added.

### Authoring/presentation/campaign

- 9×9 boards and valid authored references.
- distinct glTF scene10 exists; scene count11.
- extraction ring `(8,0)`.
- HUD M2 Round n/3, HUD M3 Round n/5.
- One→Two→Three→Four progression and 1200 base credits.
- save/Continue/Upgrade/restart remain mission-generic.

## Manual validation gate

Record `docs/validation/hpa-637.md` with:

1. continuous M1→M2→M3→M4 handoff;
2. Mission2 competing threats;
3. Mission2 immediate clear win and Round3 survival win;
4. Mission2 Gunner KO and bonus boundaries;
5. magenta Flanker scene and extraction ring;
6. player Round4 exists;
7. open-route extraction after Round4;
8. blocked-exit Round5 backstop;
9. Courier-only victory/escort non-victory;
10. early bonus;
11. save/Continue/upgrades;
12. all CI-equivalent commands.

## Expected file boundaries

New: `src/mission/enemies.rs`, `mission_two.rs`, `mission_three.rs`, `docs/validation/hpa-637.md`.

Modified: domain model/battle/enemy, mission mod/mission_one, campaign progression, presentation assets/battlefield/interaction/ui/campaign_ui, existing glTF, campaign flow/persistence/presentation tests, README, CLAUDE.md.

`src/presentation/sync.rs`, `src/app.rs`, save/session implementation, VN assets, Cargo files stay unchanged unless a concrete failing integration test proves otherwise.

## Decision summary

Keep the architecture small, but make the authored game real: Mission2 ends when the protected work is safe instead of forcing empty input; Mission3 uses deadline5 so the Courier can actually extract after player Round4; the deadline becomes the intended body-block backstop. Flanker gets a distinct checked-in scene rather than a shrunken friendly model. One round-boundary helper owns the subtle phase/round coupling, mission tests validate authoring references, and the existing positional initiative hack is removed without adding a new initiative system.