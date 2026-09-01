# HPA-524 Mission 6 Dreadnought Design

## Outcome

Ship Mission 6 as the first boss encounter while keeping the boss on the existing single-cell enemy/combat path. The fight should feel different because the Dreadnought commits large area attacks and switches to a stronger close-pressure salvo at half HP, not because Scorpius gains a boss engine.

Keep this as one HPA-524 PR. Reuse the deterministic battle state, locked enemy intents, `EliminateTarget`, `Turnabout`, authored mission openings, campaign save/progression flow, Bevy UI, and checked-in glTF. Do not add phase scripting, a boss component hierarchy, generic threshold registry, new objective shape, status framework, second battle runtime, or save migration.

## Why this is the next slice

HPA-523 is complete and was HPA-524's only blocker. HPA-524 blocks HPA-386, so Mission 6 is the next unblocked roadmap item.

The current code already supplies the required seams:

- every enemy is a normal `UnitState` on one grid cell;
- movement already calls `unit_weapon`, while `build_intent` still reads `.first()` independently and must be changed to use the same selector;
- committed attacks snapshot weapon/profile/origin/footprint and resolve against the live occupant without retargeting;
- `EliminateTarget` + `Turnabout` already ship together in Mission 4;
- regular enemies, friendly fire through committed footprints, collision, and one-cell push are proven;
- missions own board geometry, deployment, openings, dialogue, rewards, and `MissionDefinition`;
- Mission 5 already persists `MissionId::Six`;
- `Proceed` already decides authored-vs-handoff through `mission_definition(next_mission).is_some()`, while `Continue` still enumerates mission IDs explicitly;
- the checked-in `assets/models/mission_one.gltf` already carries all unit scenes and can accept one more scene without a new pipeline.

## Approaches considered

### A — One concrete Dreadnought with a derived half-HP weapon switch — selected

Add `UnitArchetype::Dreadnought`. Mission 6 gives it two ordinary enemy weapons in fixed slots. For this archetype only, `unit_weapon` selects slot 0 above half HP and slot 1 at or below half HP.

The threshold is derived from the authored unit:

```rust
unit.hp * 2 <= unit.stats.max_hp
```

Mission 6 authors 40 max HP, so the explicit threshold is 20 HP. No phase field is stored. Because the MVP has no healing, the threshold is one-way in normal play. The selected weapon also feeds the existing attack-band movement, so the same rule changes both future attack commitment and preferred range.

Mission 7 is the second concrete consumer and can justify a small shared threshold seam only if its needs overlap.

### B — Generic boss/threshold data — rejected

A `BossBehavior`, phase enum, threshold table, per-phase policy, or serialized boss metadata is more machinery than one boss needs.

### C — Parallel boss runtime or scripted phases — rejected

A boss controller, encounter script, detachable parts, invulnerability transitions, multi-tile occupancy, or callback registry would duplicate battle lifecycle rules and violate the ticket scope.

## Closed domain change

### Dreadnought archetype

Extend the closed enum with exactly one enemy:

```rust
pub enum UnitArchetype {
    Vanguard,
    Gunner,
    Interceptor,
    Rifleman,
    Striker,
    Artillery,
    Flanker,
    Bulwark,
    Controller,
    Dreadnought,
}
```

Dreadnought remains `Faction::Enemy`, occupies one cell, uses ordinary stats/damage/knockout, and remains pushable through the existing displacement rule. Do not add displacement resistance in HPA-524.

### Active enemy weapon selection

Keep `unit_weapon` as the one selector for enemy movement and change `build_intent` to call it too:

```text
Dreadnought HP 21–40 -> weapon slot 0
Dreadnought HP 0–20  -> weapon slot 1
all other enemies     -> weapon slot 0
```

A missing required slot remains a programmer/authored-data error; do not silently fall back.

### Locked threshold semantics

Crossing the threshold never mutates an already-committed intent:

```text
Round N planning: boss at 21 HP -> Graviton Salvo committed
Player phase: boss drops to 19 HP
Round N resolution: committed Graviton Salvo resolves unchanged
Round N+1 planning: Overload Salvo is selected and committed
```

No `BossPhaseChanged` event or phase banner is required. The visible transition is the next normal telegraph changing weapon name, range, expected damage, and hit/crit values.

### Movement and initiative

Dreadnought initiative is **40**, ahead of Controller 35 and the regular roster.

Later-round movement groups it with the existing attack-band enemies:

```text
Rifleman | Striker | Bulwark | Dreadnought -> attack_band_destination
```

Because movement receives the active weapon, Overload's max range 4 must visibly close pressure when the boss is five cells from its nearest player. This behavior is part of the threshold contract and gets a focused destination test.

Keep all movement/initiative matches exhaustive.

## Boss 1 — Dreadnought

Mission 6 owns the factory and weapon constants locally in `mission_six.rs`; `mission/enemies.rs` remains the shared regular-roster layer.

```text
Name       Dreadnought
HP         40
Armor       3
Move        1
Accuracy   90
Evasion     5
EN          0
Initiative 40

Weapon 207 — Graviton Salvo
Range 3–6
Cross1
Base damage 8
Hit modifier +10
Crit 5%
EN 0
No push
No counter

Weapon 208 — Overload Salvo
Range 1–4
Cross1
Base damage 10
Hit modifier +10
Crit 10%
EN 0
No push
No counter

Threshold
HP 21–40 -> Graviton Salvo
HP 0–20  -> Overload Salvo
```

## Mission 6 — Break the Dreadnought

### Product intent

The boss's committed Cross1 is both the main threat and a weapon the player can redirect onto escorts. At half HP the same boss begins closing distance because Overload's attack band is shorter.

### Board

9×9:

```text
Players
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking
(2,4) (6,4)
(2,5) (6,5)

No hazards
No explosive props

Enemies / opening
Dreadnought 61 start (4,1) -> (4,2), target Vanguard
Bulwark     62 start (0,7) -> (1,7), target Vanguard
Controller  63 start (8,7) -> (6,7), target Vanguard
Rifleman    64 start (8,6) -> (6,6), target Interceptor
```

The shared opening validator proves each row is legal. Mission-specific tests pin all exact rows.

### Opening manipulation line

The Dreadnought commits Graviton Salvo centered on Vanguard `(4,7)`; its Cross1 contains `(5,7)`.

The intended player line is:

1. Vanguard `(4,7) -> (4,5)`, vacating the Dreadnought and Controller committed target;
2. Interceptor `(5,8) -> (7,7)`;
3. Vector Pulse Controller `(6,7) -> (5,7)`;
4. resolve the unchanged enemy intents.

The Dreadnought's committed Cross1 can then roll against Controller at `(5,7)` without retargeting, while Controller's committed `(4,7)` push lands on empty space.

Baseline damage:

```text
Controller HP 9 / Armor 1
Vector Pulse normal damage at weapon level 0: 4 - 1 = 3
Graviton Salvo normal damage against Controller: 8 - 1 = 7
```

The hit remains RNG-driven. Tests pin the public geometry and use a deterministic seed sweep to prove one redirected Graviton hit reaches the existing `DamageSource::EnemyWeapon` observer and emits `OptionalObjectiveCompleted`. No friendly-fire special case is added.

### Rules and rewards

```text
Primary: EliminateTarget { target: DREADNOUGHT }
Copy: Destroy the Dreadnought; escorts may be ignored.

Bonus: Turnabout
Copy: Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.

Base reward: 800
Bonus reward: 250
Unlocks: Mission 7 handoff
```

Do not add a turn limit. Tune authored HP/damage/opening values first if manual validation shows pacing problems.

### Story

Reuse `vn/relay_nine_bg.png`, `vn/control_neutral.png`, `vn/control_alert.png`, and `vn/vanguard_neutral.png` only.

Pre-mission:

1. Control: “A Dreadnought is anchoring the line. Its main battery commits before we move.”
2. Vanguard: “Then the escorts are ammunition.”
3. Control: “Exactly. Below half integrity the battery overloads and the Dreadnought will close in.”

Aftermath:

1. Vanguard: “Dreadnought down. Their line is collapsing.”
2. Control: “One command unit remains. Mission 7 is the final push.”

## Campaign handoff

Extend `MissionId` from One–Six to One–Seven:

```text
One–Six -> authored definitions
Seven   -> terminal HPA-524 handoff; mission_definition(Seven) == None
```

`MISSION_SIX_DEFINITION.unlocks = MissionId::Seven`. `CampaignState::complete_mission` remains unchanged.

Remove the leftover per-mission `Continue` list. Keep Mission 1 special; for every later ID use the same authored-data seam as `Proceed`:

```rust
Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
Ok(id) => next_state.set(if mission_definition(id).is_some() {
    GameScreen::Upgrade
} else {
    GameScreen::NextMission
}),
```

When Six becomes authored, the existing library tests in Missions 2–5 that pin `mission_definition(Six).is_none()` move to Seven in the same task so `cargo test --lib` stays green.

Campaign integration tests then move the old Six terminal assertions to Seven, add Six as authored, and use the existing `complete_current_mission` API for Mission 6 reward/persistence coverage.

No new routing helper/table, `GameScreen`, save field, or migration layer.

## Presentation

### Boss telegraph/HUD

Do not add a boss-only HUD or phase banner. Existing intent UI already shows weapon, footprint, intended occupant, expected damage, and hit chance. Existing target-objective HUD already shows Dreadnought HP.

### Dreadnought visual

Append one distinct scene:

```text
Scene index 13: Dreadnought
Root node 70
Part nodes 71–76
Mesh/material index 13
Root authored scale 1.12
Material: Dreadnought Crimson
Base color: [0.55, 0.08, 0.12, 1.0]
```

Final counts:

```text
14 scenes
77 nodes
14 meshes
14 materials
1 embedded buffer
```

Set `MISSION_ONE_SCENE_COUNT = 14` and map `Dreadnought -> 13`.

Existing Flanker and Bulwark/Controller tests also pin old global counts and must change to 14/77/14/14. Bound the current Controller part-node loop to `.skip(64).take(6)` before appending nodes 70–76, otherwise it walks into Dreadnought nodes.

No second glTF, texture, animation, generator, under-ring, or inverse-scale compensation.

## Testing

Focused coverage must prove:

1. slot 0 at 21 HP and slot 1 at 20 HP; further damage stays slot 1;
2. committed Graviton remains unchanged across threshold crossing; a newly built intent uses Overload;
3. at 20 HP and distance 5, attack-band movement steps one cell closer so Overload reaches;
4. Dreadnought remains a normal push target;
5. Mission 6 board, roster, opening rows, objective, rewards, stats/weapons, and opening legality;
6. public opening manipulation puts Controller on `(5,7)`, leaves `(4,7)` empty, and redirected Graviton can complete Turnabout through ordinary enemy-weapon damage;
7. Dreadnought KO wins with escorts alive;
8. Missions 2–5 terminal-definition pins move from Six to Seven when Six is registered;
9. `campaign_model` treats Six as authored/Seven as terminal and base rewards through Six total 3300;
10. `campaign_flow` Continue and Proceed treat Six as authored and Seven as terminal;
11. Mission 6 completion uses `complete_current_mission`, advances once to Seven, and covers 800/250 rewards;
12. Seven save round-trip preserves upgrades/credits;
13. old/new glTF structural tests agree on 14/77/14/14/1 and Controller loop is bounded;
14. all existing Missions 1–5 and campaign/save/presentation suites remain green.

## Manual validation

Record HPA-524 evidence in `docs/validation/hpa-524.md`:

- start Mission 6 from the real campaign flow after Mission 5;
- confirm Graviton readability and escort redirection;
- cross 21+ -> <=20 after commitment and confirm current telegraph stays Graviton;
- confirm next planning shows Overload and closes from range 5 into range 4;
- confirm normal boss push;
- defeat Dreadnought with an escort alive and confirm immediate victory;
- finish aftermath/reward/upgrade, return to title, Continue, and confirm persisted Mission 7 handoff;
- record encounter length and tune authored values only if clearly necessary.

## Scope guardrails

No multi-tile boss, parts, invulnerability, cinematic battle scene, unique boss runtime, threshold registry, phase scripting, generic behavior policies, displacement immunity, new status effect, new objective/optional shape, new hazard/prop type, new progression track, new save field, save migration, dependency/crate, second boss, Mission 7 content, new VN art, second glTF, or asset pipeline.

Mission 7 is deliberately the second threshold consumer. Generalize only behavior proven by both encounters.