# HPA-637 Missions 2–3 and Flanker Design

## Context

HPA-635 is complete on `main`; the baseline for this work is `ca4a281cbb72261429fe6a5247816fa25aacff62`. Scorpius already has the complete Title → VN → Briefing → Battle → Aftermath → Upgrade loop, save/continue, credits/upgrades, pilot skills, and one authored Mission 1 definition. `MissionId::Two` is currently only a saved handoff.

HPA-637 is the next unblocked Scorpius issue. It expands that validated loop to three complete missions and introduces the fourth regular enemy, Flanker. This is a bounded architectural/content slice: two missions need a small objective seam and one enemy needs deterministic objective-aware movement, but there is still no need for a generic objective framework, behavior tree, scripting layer, or new combat subsystem.

The delivery remains **one ticket = one PR**. This draft planning PR is the implementation PR for HPA-637.

## Goals

1. Make Missions 2 and 3 fully playable through the existing campaign/save/upgrade loop.
2. Make Mission 2 win/fail from a protect/survive condition rather than destroy-all.
3. Make Mission 3 win/fail from interception/extraction/deadline rather than destroy-all.
4. Add Flanker as a visibly/mechanically distinct fourth regular enemy: fast, evasive, fragile, objective-seeking, one attack.
5. Show primary + bonus objective in briefing, live HUD, and result overlay.
6. Keep bonuses credit-only and never required for progression.
7. Preserve Mission 1's exact validated opening threats and committed-intent semantics.
8. Keep everything typed, deterministic, single-crate, and Bevy-free in `domain`.

## Non-goals

Do not add a neutral faction/objective unit, objective callback/trait registry, behavior tree/utility-AI framework, pathfinding package, stealth, teleport, a new initiative system, new playable units, deployment selection, new hazard types, status effects, bosses, mission select, branching, difficulty, RON/JSON mission authoring, another crate, new VN art, a new glTF pipeline, or save migration/backward-compatibility code.

Existing saves containing Mission 1/2 enum values can deserialize naturally after adding later variants; no migration branch is needed.

## Approaches considered

### Generic objective/AI framework

Rejected. HPA-637 has exactly three primary objective shapes and one new archetype. A framework would add indirection before another consumer exists and contradict the ticket's “only seams these missions consume” rule.

### New neutral protected objective

Rejected. Mission 2 can protect the existing fragile Gunner, immediately reusing movement, HP, targeting, locked telegraphs, Guard/Evade/Counter, and Aegis. A neutral role/faction would force changes through activation, selection, combat targeting, HUD, and victory rules for no current benefit.

### Closed mission rules + existing squad target + one Flanker planner branch

Chosen. `BattleState` receives one closed rules row, Mission 1 opening data moves out of hard-coded enemy logic, and Flanker gets only the two positioning policies Missions 2/3 consume.

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

These enums are the three authored shapes we have, not an extensibility framework.

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

## 3. Terminal semantics stay in `BattleState`

`check_terminal_state` remains the only place that seals result/phase and clears terminal transient state. It evaluates `MissionRules::primary`.

Global rule: if no player unit is alive, defeat.

### Mission 1: eliminate all

`EliminateAllEnemies` preserves current behavior: victory when no enemy remains and at least one player is alive.

### Mission 2: protect Gunner through Round 3

`ProtectThroughRound { target: GUNNER, round: 3 }` means:

- Gunner KO → immediate defeat.
- Victory only when `phase == EnemyPlanning && round >= 3` with Gunner alive.
- Killing every enemy early does not win.
- Other player losses are allowed unless they cause global squad wipe.

Using the phase boundary avoids a new “enemy phases completed” counter: Round-3 victory happens only after all Round-3 committed enemy intents resolve and the state returns to EnemyPlanning.

### Mission 3: intercept Courier

`InterceptBeforeEscape { target: COURIER, escape: (8,2), deadline_round: 4 }` means:

- Courier KO → immediate victory even with escorts alive.
- Courier at `(8,2)` → defeat.
- Courier alive at `EnemyPlanning` with `round >= 4` → defeat.
- Killing escorts without Courier does not win.

The Round-4 fallback prevents indefinite body-block/stall if the exact extraction cell is occupied.

## 4. Bonus completion stays one bit

- `Turnabout`: mark once on the existing qualifying enemy/environment damage event.
- `ProtectTargetAtHalfHp`: on Mission 2 victory, succeed if `target.hp * 2 >= target.stats.max_hp`.
- `VictoryByRound { round: 2 }`: on Mission 3 victory, succeed if `battle.round() <= 2`.

When a terminal victory satisfies a not-yet-complete terminal bonus, emit `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat does not newly award a terminal-only bonus.

## 5. Mission-authored opening data replaces Mission-1 hardcoding

`enemy.rs` currently knows Mission 1 positions/targets. Replace that with `MissionRules::opening_plan`:

- on round 0, lookup enemy row by `UnitId`, move directly to its authored destination, or stay if absent;
- when committing round-0 intent, use the row's living target unit position as forced center when present;
- later rounds still use `choose_enemy_destination`.

Authored opening placement remains direct scripted movement, matching current Mission 1 semantics; it does not become pathfinding or an activation.

Mission 1 opening remains exactly:

```text
Rifleman L  -> (2,5), target Gunner
Rifleman R  -> (6,5), target Interceptor
Striker     -> (4,6), target Vanguard
Artillery   -> (4,0), target Vanguard
```

Existing tests continue to pin positions, attacker order, intended occupants, and mortar footprint.

## 6. Shared regular-enemy catalog

Create `src/mission/enemies.rs`, mirroring the already-justified shared player `squad.rs` boundary. It owns fixed constructors/weapon specs; each mission still owns IDs, names, board, deployment, roster, opening, rules, dialogue, and rewards.

| Enemy | HP | Armor | Move | Accuracy | Evasion | Weapon |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rifleman | 9 | 1 | 2 | 72 | 5 | Service Rifle |
| Striker | 12 | 2 | 2 | 78 | 10 | Shock Claw |
| Artillery | 10 | 1 | 1 | 90 | 0 | Siege Mortar |
| **Flanker** | **8** | **0** | **4** | **82** | **30** | **Skirmish Carbine** |

Existing three enemy weapon values stay unchanged. Flanker has exactly one new weapon:

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

## 7. Flanker behavior stays an explicit branch in `enemy.rs`

Add `UnitArchetype::Flanker`.

### Protect mission

For `ProtectThroughRound`, Flanker uses the protected unit's current position as goal. Score reachable cells by:

1. distance to its weapon's legal range band around the goal;
2. Manhattan distance to the goal;
3. more open orthogonal neighbors first;
4. `y`, then `x`.

When choosing a legal attack footprint, a Flanker prefers one containing the protected target before the existing threatened-count/player-priority ordering.

### Interception mission

If the Flanker is the designated `InterceptBeforeEscape` target, score reachable cells by:

1. Manhattan distance to extraction;
2. more open orthogonal neighbors first;
3. `y`, then `x`.

It still commits a normal Skirmish Carbine intent after moving. No RNG is added to movement/target selection, and committed intent never retargets during the player phase.

Other archetypes retain current later-round behavior. No behavior-policy abstraction is introduced.

## 8. Mission 2 — Hold Relay Nine

Create `src/mission/mission_two.rs`.

### Definition

```text
Title: Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3.
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

### 9×9 board

```text
Player deployment
Vanguard    (3,7)
Gunner      (4,6)  protected
Interceptor (5,7)

Blocking: (3,3), (5,3), (2,6), (6,6)
Hazards:  (1,5), (7,5)
Explosive: (6,4), HP 4
```

### Enemy roster/opening

```text
Rifleman  id 21, starts (2,2), opening -> (2,4), target Vanguard
Striker   id 22, starts (4,3), opening -> (4,5), target Gunner
Artillery id 23, starts (4,0), opening -> (4,0), target Gunner
Flanker   id 24, starts (8,4), opening -> (5,5), target Interceptor
```

All forced targets are legal from their opening destinations. This is deliberately **not** four attacks on Gunner: the opening creates readable competing threats across the squad while two dangerous locks pressure the protected unit. Reactions matter on all three mechs; the player must decide how much movement/Aegis/action economy to spend protecting Gunner. From later rounds onward, Flanker begins its objective-aware chase toward Gunner.

There are no reinforcements/waves. If every enemy dies early, the player still must complete the protection duration.

### VN copy

Reuse only existing `relay_nine_bg.png`, Control portraits, and Vanguard portrait.

Pre-mission:

1. Control: `Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.`
2. Vanguard: `Then Gunner stays standing. We move around the locks, cover the weak angles, and hold.`
3. Control: `New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.`

Aftermath:

1. Vanguard: `Uplink complete. Relay Nine can finally hand us the enemy route data.`
2. Control: `It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.`

## 9. Mission 3 — Cut the Courier

Create `src/mission/mission_three.rs`.

### Definition

```text
Title: Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 4.
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
        escape: GridPos::new(8, 2),
        deadline_round: 4,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
}
```

### 9×9 board

```text
Player deployment
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking: (4,3), (4,4), (4,5)
Hazard:   (2,5)
Explosive: (6,3), HP 4
Extraction: (8,2)
```

The extraction cell is a logical domain objective, not a new prop type. Presentation renders a persistent white objective ring on `(8,2)` using the existing `ring_mesh` + `intended_target` material so the deadline/route is visible on the board.

### Enemy roster/opening

```text
Courier   id 31, Flanker, starts/stays (0,6), no forced target
Rifleman  id 32, starts (3,2), opening -> (3,4), target Vanguard
Striker   id 33, starts (6,6), opening -> (5,7), target Interceptor
```

Courier is the strategic target; the escorts create readable locked threats but never gate victory. The start `(0,6)` to extraction `(8,2)` is twelve Manhattan steps before occupancy/wall effects, giving a movement-4 Courier roughly three later-round movement passes to threaten extraction. Round 4 remains the hard fallback deadline.

### VN copy

Pre-mission:

1. Control: `Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.`
2. Vanguard: `We cut across and stop it. Escorts are secondary — the Courier is the mission.`
3. Control: `Extraction is at the east marker. If it gets out, or Round 4 closes, the data is gone.`

Aftermath:

1. Vanguard: `Courier down. The route keys are intact.`
2. Control: `Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.`

## 10. Mission dispatch grows only through the next handoff

`MissionId` becomes `One, Two, Three, Four`, with a small `number()` helper for handoff copy.

`mission_definition` returns authored rows for One/Two/Three and `None` for Four. Do not replace the match with a registry, collection, or plugin.

## 11. Continuous campaign routing

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

## 12. Objective-generic HUD/results/rewards

Keep `MissionDefinition.primary_objective` / `optional_objective` as authored human copy. Presentation appends progress from `BattleState::rules()`:

- Eliminate: enemy count remaining.
- Protect: current/required round + protected unit HP.
- Intercept: current/deadline round + Courier Manhattan distance to extraction.
- Turnabout: Complete/Not yet.
- Half-HP: On track/Missed.
- Victory-by-round: Available/Missed; terminal state uses `optional_complete`.

Result overlay accepts `MissionResult + MissionDefinition`:

```text
MISSION COMPLETE | MISSION FAILED
<mission title>
PRIMARY  <primary text> · Complete/Failed
BONUS    <bonus text> · Achieved/Missed
```

Aftermath reward copy uses `Bonus +...`, not `Turnabout +...`.

## 13. Flanker/extraction visuals reuse existing rendering assets

Do not modify the checked-in glTF.

- `scene_index(Flanker) = 2` (existing Interceptor fast silhouette).
- Flanker model scale `0.62`; all other units remain `0.72`.
- Spawn a persistent Flanker under-ring as a **child of the unit visual entity**, using existing `ring_mesh` and `telegraph_edge` material. Childing makes it follow movement automatically; no new sync framework/component is needed.
- `apply_unit_transforms` must use the same `unit_scale(archetype)` helper so per-frame sync does not reset Flanker to `0.72`.
- For `InterceptBeforeEscape`, spawn one static extraction ring under `PresentationRoot` at the authored escape cell using existing `ring_mesh` + `intended_target` material.
- Rename touched debug root text `Mission 1 Presentation` → `Mission Presentation`.

Enemy rotation + fast silhouette + under-ring distinguish Flanker from the player's Interceptor; movement 4/evasion 30/low durability/objective-seeking distinguish it mechanically.

## 14. Rewards/progression tuning

Base rewards alone are 300 + 400 + 500 = **1200 credits** through Mission 3. Optional rewards are 100 + 100 + 150. Normal progression can therefore buy useful 200/400-level upgrades without requiring bonuses or grinding.

No new progression system is needed.

---

## Testing strategy

### Domain/objective

- Mission 1 eliminate-all and Turnabout remain unchanged.
- Mission 2 enemy-clear alone does not win.
- Mission 2 Gunner KO fails.
- Mission 2 wins only at EnemyPlanning after Round 3 with Gunner alive.
- Mission 2 half-HP boundary (`hp * 2 >= max_hp`) is achieved; below it is missed.
- Mission 3 Courier KO wins while escorts live.
- Mission 3 escort clear alone does not win.
- Mission 3 extraction and Round-4 deadline each fail.
- Mission 3 Round-2 KO earns bonus; Round-3 KO does not.
- Terminal-only bonus event precedes `MissionCompleted` once.

### Enemy planner

- Mission 1 exact authored opening regression remains.
- Mission 2 opening intended occupants are Vanguard/Gunner/Gunner/Interceptor.
- Later Mission 2 Flanker movement/intent prioritizes protected Gunner and uses open-neighbor tie-break.
- Mission 3 Courier destination reduces extraction distance rather than chasing a normal player target.
- No RNG is introduced to destination/target ordering.

### Mission authoring

Pin exact board, deployment, roster, rules, rewards/copy/unlock for Missions 2/3 and prove current upgrades still project through `build_player_squad` once.

### Campaign/presentation

- M1 → M2 → M3 normal completion advances to Four with 1200 base credits.
- bonus changes credits only.
- save/load round-trips MissionId Four + upgrades.
- Continue routes One/story, Two/Three/Upgrade, Four/handoff.
- Upgrade Proceed routes authored Two/Three to story and Four to handoff.
- battle entry/restart uses current definition builder for M2/M3.
- briefing/HUD/result show both objective texts and dynamic progress.
- Flanker scene/scale + extraction marker helper paths are covered by pure tests where practical; manual play confirms rendered rings.

## Manual validation gate

Record `docs/validation/hpa-637.md` with:

1. continuous M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff;
2. M2 opening competing threats visible;
3. M2 enemy-clear does not win, Round-3 survive does, Gunner KO fails;
4. M2 bonus achieved/missed;
5. M3 Courier/under-ring and extraction ring visible;
6. M3 Courier routes toward extraction while committed telegraphs remain locked;
7. M3 wins with escorts alive;
8. M3 extraction + deadline failures;
9. M3 early bonus achieved/missed;
10. save/quit/Continue and upgrades retained before M2 and M3;
11. full CI-equivalent commands pass.

If either mission needs slow empty-round stalling or gives no meaningful response window, tune authored positions/terrain/stats/rewards inside this PR. Do not add reinforcements, wave systems, or another combat subsystem to solve content tuning.

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
- `src/presentation/battlefield.rs`
- `src/presentation/sync.rs`
- `src/presentation/interaction.rs`
- `src/presentation/ui.rs`
- `src/presentation/campaign_ui.rs`
- `tests/presentation_app.rs`
- `README.md`
- `CLAUDE.md`

`src/app.rs`, save/session implementation, glTF/PNG assets, Cargo files should remain unchanged unless a concrete failing integration test proves otherwise.

## Decision summary

Extend what already works: three closed primary objective shapes, three closed bonus shapes backed by one bit, one authored opening slice, one small regular-enemy catalog, and one Flanker branch in the deterministic planner. Mission 2 protects Gunner instead of inventing an objective unit. Mission 3 wins on Courier alone and visibly marks extraction. Existing screens/save/combat remain the vocabulary for the entire slice.
