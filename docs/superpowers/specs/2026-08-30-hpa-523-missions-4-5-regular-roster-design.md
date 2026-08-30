# HPA-523 Missions 4–5 and Regular Enemy Roster Design

## Outcome

Extend Scorpius from the completed Mission 3 handoff through Missions 4–5, add the Bulwark and Controller as the fifth and sixth regular enemy archetypes, and leave Mission 6 as the next campaign handoff.

Keep this as one HPA-523 PR. Reuse the current deterministic battle state, authored mission rows, one-cell displacement rules, campaign save/progression flow, Bevy UI, and checked-in glTF. Do not add an objective framework, AI policy layer, status system, physics, scripting format, new hazard type, or asset pipeline.

## Why this is the next slice

HPA-637 is complete and was HPA-523's only blocker. HPA-523 blocks HPA-524, so this is the first unblocked roadmap item after Missions 2–3.

The current code already supplies nearly every mechanic HPA-523 needs:

- push, collision, hazards, and explosive cross damage;
- fixed enemy openings followed by deterministic movement/intent planning;
- locked intents that hit the current occupant of a committed footprint without retargeting;
- closed primary/optional objective enums;
- mission-owned boards, deployment, dialogue, rewards, and builders;
- campaign save/upgrade flow through the Mission 4 handoff;
- one checked-in glTF containing the battlefield unit/prop scenes.

There is no displacement-resistance rule on `main`. HPA-523 must therefore not invent one for Bulwark. Bulwark is durable because of HP/armor and authored lane placement, but existing push rules still move it. If Mission 6's boss becomes the first concrete need for displacement resistance, HPA-524 can add the smallest rule then.

## Approaches considered

### A — One exact new objective plus explicit enemies — selected

Add only `PrimaryObjective::EliminateTarget { target }`, then implement Bulwark and Controller as explicit additions to the current closed enemy roster. Mission 4 uses the target objective to make its environmental puzzle end when the Gate Bulwark falls; Mission 5 remains `EliminateAllEnemies` and makes locked artillery footprints the puzzle.

This gives Mission 4 a distinct objective without introducing callbacks, traits, registries, generic objective composition, or a reusable AI policy layer.

### B — Reuse `EliminateAllEnemies` for both missions — rejected

This would minimize code changes, but Mission 4 would reward clearing escorts rather than solving the environmental breach. It also weakens the ticket's explicit instruction to avoid destroy-all when a clearer authored objective exists.

### C — Generic objective/AI/status frameworks — rejected

HPA-523 has one new objective shape, one new movement preference, and one push weapon. A reusable objective registry, behavior policies, status effects, or displacement capability model would add more machinery than product value for the seven-mission MVP.

## Closed domain changes

### Primary objective

Extend the existing closed enum by exactly one variant:

```rust
pub enum PrimaryObjective {
    EliminateAllEnemies,
    ProtectThroughRound { target: UnitId, round: u16 },
    InterceptBeforeEscape {
        target: UnitId,
        escape: GridPos,
        deadline_round: u16,
    },
    EliminateTarget { target: UnitId },
}
```

`BattleState::check_terminal_state` evaluates it directly:

```text
target knocked out -> victory immediately, even if escorts survive
all player units knocked out while target lives -> defeat
otherwise -> continue
```

No objective callback, trait, registry, runtime objective validation layer, or serialized objective payload is added.

### Optional objectives

Do not add another optional shape. Reuse:

- Mission 4: `Turnabout`;
- Mission 5: `VictoryByRound { round: 4 }`.

Turnabout keeps its current trigger definition: enemy damage caused by enemy fire, collision, hazard, or explosion completes it. Mission 4's copy presents that existing rule as an environmental challenge.

### Shared weapon reach/alignment rule

The crate already has one exact reach rule in `domain::combat::weapon_reaches`:

```text
min_range <= Manhattan distance <= max_range
and, when weapon.push, attacker/target share x or y
```

Make that helper crate-visible and reuse it for enemy target generation, authored-opening validation, and forced opening intent validation. Do not add a second `push_target_aligned` helper.

### Committed push semantics after attacker displacement

Controller is the first enemy with `push: true`. The player can displace it after its intent is committed, so commit-time alignment cannot be the only invariant.

Locked rule:

> A committed enemy push attack keeps its committed footprint. If it hits the current occupant after the attacker has been displaced so the live attacker/target positions are no longer aligned, the attack still deals its normal damage but skips the push.

This preserves the game's locked-intent model: moving the attacker does not retarget or cancel the footprint, but an impossible live displacement does not become a domain error.

Implementation stays local to enemy attack resolution: after a hit, call `resolve_push` only when the live attacker and current target still satisfy `weapon_reaches` for the push weapon. A lost-alignment push produces damage events only; it must not return `PushTargetNotAligned` from normal gameplay.

Do **not** add the proposed generic `phase = Player` recovery on every enemy-resolution error. Enemy resolution mutates incrementally; resetting only the phase after a later intent fails would let a retry replay already-applied intents. HPA-523 fixes the concrete expected Controller path so it is non-erroring. Unexpected programmer/data errors remain errors rather than introducing partial-replay semantics or a transactional battle framework.

### Authored opening legality

Mission 1, 2, and 3 already duplicate the same opening-legality test. HPA-523 is the point to extract it because Missions 4 and 5 would otherwise be copies four and five.

Add one test-only helper in `src/mission/mod.rs`:

```rust
#[cfg(test)]
pub(crate) fn assert_opening_plan_is_legal(battle: &BattleState)
```

It checks every authored opening row:

- opening count equals the number of enemy units;
- opener exists and is `Faction::Enemy`;
- destination is within the opener's authored movement allowance, in bounds, non-blocking, non-hazard, and not initially occupied by another unit;
- optional target, when present, exists and is `Faction::Player`;
- opener has a first weapon and `weapon_reaches(weapon, opening.destination, target.position)` is true for every targeted opening, including push alignment.

Mission-specific tests still pin exact IDs/destinations/targets, but call this shared helper for legality. Replace the existing Mission 1/2/3 copied legality bodies in the same task.

### Exhaustive enemy planning matches

When Bulwark and Controller are added, `choose_enemy_destination` and `initiative` must no longer hide future archetypes behind `_` fallbacks.

Use explicit player-only fallbacks:

```text
Vanguard | Gunner | Interceptor -> stay put / initiative 0
```

Every enemy archetype is named explicitly. A future seventh regular/boss archetype then creates a compiler error until its movement/initiative behavior is consciously chosen. Keep a Bulwark later-round movement regression as behavior coverage even with the exhaustive match.

## Regular enemy roster

Extend `UnitArchetype` to the final six regular enemies:

```text
Rifleman
Striker
Artillery
Flanker
Bulwark
Controller
```

Player archetypes remain unchanged.

## Bulwark

Bulwark is a slow armored body-blocker, not a source of aura or zone-of-control rules.

Locked values:

```text
HP        16
Armor      4
Move       1
Accuracy  76
Evasion    0
EN         0

Weapon ID 205
Bastion Cannon
Range 1–3
Single target
Base damage 6
Hit modifier +0
Crit 5%
EN 0
No push
No counter
```

Behavior:

- use the existing attack-band movement used by Rifleman/Striker;
- Move 1 plus authored blocking cells/placement creates route pressure;
- use normal target selection;
- remain pushable under the existing one-cell displacement rule;
- no resistance flag, mass value, ZOC, guard aura, taunt, or shield subsystem.

Initiative: **15**.

## Controller

Controller is a low-damage displacement enemy that changes board geometry through the existing push path.

Locked values:

```text
HP         9
Armor      1
Move       2
Accuracy  82
Evasion   15
EN         0

Weapon ID 206
Impulse Projector
Range 2–4
Single target
Base damage 3
Hit modifier +10
Crit 0%
EN 0
Push 1 through existing resolve_push
No counter
```

The enemy weapon is named **Impulse Projector**, not Vector Projector, to avoid confusion with the Interceptor's existing **Vector Pulse** push weapon.

Behavior:

- later-round movement considers current reachable cells plus origin;
- first prefer candidates from which at least one living player is inside range 2–4 and aligned on the same row or column;
- among legal push-lane candidates, minimize distance to the normal attack band, then nearest player distance, then `(y, x)` for deterministic tie-breaking;
- if no push lane is reachable, fall back to the current attack-band destination logic;
- dynamic target generation and authored forced targets both reuse `weapon_reaches`, so no diagonal push intent can be committed;
- if player displacement breaks alignment after commitment, resolution becomes damage-only as specified above.

No pull direction, persistent displacement status, stun, silence, slow, root, generic crowd-control descriptor, or behavior policy object is added.

Initiative: **35** so Controller resolves before the ordinary regular attackers when its footprint still contains a unit.

Final initiative table:

```text
Controller 35
Striker    30
Flanker    25
Rifleman   20
Bulwark    15
Artillery  10
```

## Mission 4 — Breach the Gate

### Product intent

Mission 4 teaches that the environment can be the primary damage plan. The player only needs to destroy the Gate Bulwark; escorts may be ignored.

### Board

9×9 board.

```text
Players
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking
(2,4) (6,4)
(2,5) (6,5)

Hazard
(4,3)

Explosives, HP 4 each
(3,4)
(5,4)

Enemies / opening
Bulwark 41    start (4,5) -> (4,4), target Vanguard
Controller 42 start (0,7) -> (1,7), target Vanguard
Rifleman 43   start (8,6) -> (6,6), target Interceptor
```

Opening legality is intentional:

- Bulwark `(4,4)` to Vanguard `(4,7)` is range 3;
- Controller `(1,7)` to Vanguard `(4,7)` is range 3 and row-aligned;
- Rifleman `(6,6)` to Interceptor `(5,8)` is range 3.

The shared opening validator proves these constraints in addition to each mission's exact authoring assertions.

### Rules and rewards

```text
Primary: EliminateTarget { target: BULWARK }
Copy: Destroy the Gate Bulwark; escorts may be ignored.

Bonus: Turnabout
Copy: Chain Reaction: damage any enemy with enemy fire, collision, hazard, or explosion.

Base reward: 600
Bonus reward: 150
Unlocks: Mission 5
```

### Authored environmental solutions

The board preserves both concrete opening-round opportunities:

1. **Explosion:** Gunner at `(3,8)` can Rail Rifle the explosive at `(3,4)` at Manhattan range 4. Its existing cross explosion includes the Bulwark at `(4,4)`.
2. **Push into hazard:** Vanguard can move `(4,7) -> (4,6) -> (4,5)`, then use Repulsor Ram on the Bulwark at `(4,4)`. Existing push sends the Bulwark to hazard `(4,3)`, applying normal weapon damage followed by hazard damage.

The mission does not require a bespoke environment-kill counter. `Turnabout` supplies the optional proof that enemy/environment damage was used, while the target objective ends the battle whenever the Bulwark is destroyed by any legal method.

### Dialogue

Reuse only existing `relay_nine_bg.png`, Control portraits, and `vanguard_neutral.png`.

Pre-mission:

1. Control: “The ridge gate is sealed by a Bulwark. Its armor is built for direct fire; the fuel cells and hazard trench around it are not.”
2. Vanguard: “So we stop treating the battlefield like scenery.”
3. Control: “Breach the Bulwark. Ignore the escorts if you can make the board do the work.”

Aftermath:

1. Vanguard: “Gate's open. Their own position did more damage than our guns.”
2. Control: “Good. Long-range batteries are waiting on the far side.”

## Mission 5 — Crossfire Break

### Product intent

Mission 5 makes locked artillery intents both the main threat and a meaningful weapon the player can redirect by moving units and displacing an enemy into already-committed footprints.

### Board

9×9 board.

```text
Players
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking
(1,4) (7,4)
(1,5) (7,5)

No new hazard or prop type.

Enemies / opening
Artillery 51  start/stay (3,0), target Gunner
Artillery 52  start/stay (7,2), target Vanguard
Bulwark 53    start (0,7) -> (1,7), target Vanguard
Controller 54 start (3,5) -> (3,6), target Gunner
Flanker 55    start (8,7) -> (6,7), target Interceptor
```

Opening legality is intentional:

- Artillery 51 to Gunner is range 8;
- Artillery 52 to Vanguard is range 8;
- Bulwark `(1,7)` to Vanguard `(4,7)` is range 3;
- Controller `(3,6)` to Gunner `(3,8)` is range 2 and column-aligned;
- Flanker `(6,7)` to Interceptor `(5,8)` is range 2.

### Rules and rewards

```text
Primary: EliminateAllEnemies
Copy: Break the assault and destroy all enemies.

Bonus: VictoryByRound { round: 4 }
Copy: Rapid Break: win by the end of Round 4.

Base reward: 700
Bonus reward: 200
Unlocks: Mission 6 handoff
```

### Locked artillery manipulation setup

At the first player phase:

- Artillery 51 has committed a Cross1 footprint centered on Gunner `(3,8)`, including `(3,7)`.
- Artillery 52 has committed a Cross1 footprint centered on Vanguard `(4,7)`, also including `(3,7)`.
- Controller 54 stands at `(3,6)` and has committed its Single push footprint on Gunner's original `(3,8)` cell.
- Bulwark 53 holds the left lane at `(1,7)`.

The deterministic player line remains the same exact-fit movement puzzle:

1. move Gunner from `(3,8)` to safe cell `(2,7)`;
2. move Vanguard `(4,7) -> (4,6) -> (4,5) -> (3,5)`;
3. Repulsor Ram Controller 54 from `(3,6)` to `(3,7)`;
4. resolve the already-locked enemy intents.

Controller resolves first at initiative 35, but its committed `(3,8)` cell is now empty, so that attack lands harmlessly. Both Artillery Cross1 footprints still contain the displaced Controller at `(3,7)` and can hit it without retargeting.

The payoff is explicit and falsifiable:

```text
Controller HP 9 / Armor 1
Repulsor Ram normal damage: 5 - 1 = 4
Each Siege Mortar normal damage: 6 - 1 = 5
Each mortar hit chance: 90 + 5 - 15 = 80%
```

After the Ram, **one** mortar hit is enough to KO the Controller (4 + 5 = 9). Both mortar hits total 14 damage including the Ram. The setup also vacates two dangerous artillery targets, so even a miss still rewards manipulating committed fire rather than simple damage optimization.

Tests pin the real committed footprints, the public movement paths, the Controller displacement, the 4/5 damage previews, and Artillery attack rolls targeting Controller at `(3,7)`. They do not add an artillery-friendly-fire special case.

### Dialogue

Reuse the existing VN assets only.

Pre-mission:

1. Control: “Two siege batteries have already locked firing solutions. Their shots will not retarget.”
2. Vanguard: “Then every red footprint is also a weapon we can aim.”
3. Control: “Exactly. Break the assault before they settle into a second firing line.”

Aftermath:

1. Vanguard: “Both batteries are down. Their crossfire did half the work for us.”
2. Control: “Regular forces are broken. What comes next is heavier.”

## Campaign progression

Grow the closed mission ID set once:

```rust
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}
```

Authored definitions exist for One through Five. Six is the terminal HPA-523 handoff and returns `None` from `mission_definition` until HPA-524 authors Mission 6.

Routing:

```text
Continue One -> PreMissionStory
Continue Two/Three/Four/Five -> Upgrade
Continue Six -> NextMission handoff
Proceed from Upgrade -> PreMissionStory when mission_definition(next_mission) exists
```

Rewards remain additive with the current economy:

```text
Base through M5: 300 + 400 + 500 + 600 + 700 = 2500
Maximum optional through M5: 100 + 100 + 150 + 150 + 200 = 700
Maximum total through M5: 3200
```

No new upgrade track, price curve, save slot, save migration, or compatibility handling is added. Existing pre-release saves may break if the enum layout changes; backward compatibility is explicitly out of scope.

## Presentation

### Objective HUD

Add one `ObjectiveTrackSnapshot::Target` form for `EliminateTarget`:

```rust
Target {
    name: &'static str,
    hp: i16,
    max_hp: i16,
}
```

Pin the rendered tracker copy so it cannot read like a protect objective:

```text
TARGET {name} HP {hp}/{max_hp}
```

Mission 4 therefore renders `TARGET Gate Bulwark HP 16/16` at full health.

`EliminateAllEnemies` becomes the only primary whose main HUD line appends `· N remaining`. This intentionally corrects the existing Mission 2/3 copy: Protect/Intercept keep their own HP/distance tracker and no longer imply that clearing every enemy is the win condition. Add explicit Mission 2 and Mission 3 snapshot/string regressions for the new copy.

No mission-specific HUD system is introduced.

### Unit visuals

Append two scenes to `assets/models/mission_one.gltf`; do not create another asset file or runtime generation path.

Current indices end at Flanker scene 10 / node 55 / mesh 10 / material 10. Append:

```text
Scene 11: Bulwark
Root node 56
Children 57–62
Mesh 11
Material 11: “Bulwark Ochre”
Material baseColorFactor [0.78, 0.38, 0.08, 1.0]
Root scale [0.88, 0.88, 0.88]

Scene 12: Controller
Root node 63
Children 64–69
Mesh 12
Material 12: “Controller Cyan”
Material baseColorFactor [0.08, 0.72, 0.86, 1.0]
Root scale [0.72, 0.72, 0.72]
```

Each six-part child set reuses the exact transforms of the existing Flanker children 50–55 and points to its own new mesh. Both new meshes reuse the existing cube POSITION/NORMAL accessors and the existing single embedded buffer. Final counts:

```text
scenes 13
nodes 70
meshes 13
materials 13
buffers 1
```

`scene_index` maps Bulwark to 11 and Controller to 12. Keep the existing shared world transform scale; no per-archetype presentation scale table is added.

## Required automated coverage

### Domain

- `EliminateTarget`: target KO wins with escorts alive; player wipe loses while target lives.
- Bulwark factory and Bastion Cannon exact stats.
- Controller factory and Impulse Projector exact stats/name.
- Controller later movement chooses a legal aligned push lane when one exists and falls back deterministically when none exists.
- Dynamic and authored push intents reuse `weapon_reaches`; no diagonal push center can be committed.
- Commit a Controller push intent, displace Controller perpendicular to its committed lane during the player phase, then resolve: resolution returns `Ok`, the committed attack still rolls/deals damage on a hit, no `UnitPushed` occurs when live alignment is lost, and the battle advances normally instead of remaining in `EnemyResolution`.
- Bulwark has a later-round regression proving its attack-band branch leaves origin when a better Move-1 cell exists.
- `choose_enemy_destination` and `initiative` have no wildcard archetype fallback; only the three player archetypes explicitly stay/return initiative 0.
- Initiative table includes Controller 35 and Bulwark 15 while preserving 30/25/20/10 existing order.
- Existing push/collision/hazard/explosion regressions remain green.

### Authored openings

- one shared `assert_opening_plan_is_legal` is called by Mission 1–5 tests;
- existing Mission 1/2/3 copied legality bodies are removed;
- exact mission-specific opening rows remain separately pinned;
- every targeted opening is in weapon range and every push opening is aligned.

### Mission 4

- exact 9×9 board, deployment, terrain, props, enemy IDs, rules, rewards, dialogue, and opening rows;
- Gunner can preview/attack the `(3,4)` explosive and the explosion footprint contains Bulwark;
- Vanguard can reach `(4,5)` and existing push sends Bulwark `(4,4) -> (4,3)` onto hazard;
- destroying only Bulwark wins with escorts alive;
- environmental/enemy damage completes Turnabout.

### Mission 5

- exact 9×9 board, deployment, enemy IDs, rules, rewards, dialogue, and revised opening rows;
- both opening Artillery intents contain `(3,7)` after `begin_round`;
- Gunner can vacate to `(2,7)` and Vanguard can reach `(3,5)` through public movement;
- existing push geometry sends Controller `(3,6) -> (3,7)`;
- Repulsor Ram previews 4 normal damage against Controller; Siege Mortar previews 5 normal damage against Controller;
- Controller's committed `(3,8)` hit lands on empty space after Gunner vacates;
- resolving both committed Artillery intents rolls against Controller at `(3,7)` without retargeting;
- victory by Round 4 earns Rapid Break; later victory does not.

### Campaign/presentation

- inline `src/presentation/ui.rs` tests cover `EliminateTarget` projection/formatting before Mission 4 exists;
- Mission 2/3 HUD regressions confirm their main objective lines no longer append enemy count;
- Task 5 integration coverage uses real Mission 4 to pin `TARGET Gate Bulwark HP ...`;
- Bulwark/Controller scene indices and glTF structure/counts are pinned;
- One -> Two -> Three -> Four -> Five -> Six progression is continuous;
- base rewards through Mission 5 total 2500;
- upgrade purchases remain persisted across Mission 4/5 entry and Mission 6 handoff;
- Continue routes Four/Five to Upgrade and Six to the handoff screen.

## Risks

### Displaced committed pusher

This is the highest-risk new rule because Controller is the first enemy push weapon while the player already has three ways to displace enemies. A perpendicular player push after commitment must not turn enemy resolution into `PushTargetNotAligned` or strand the phase. The damage-only-on-lost-alignment rule and full resolve regression are mandatory Task 2 coverage.

### Mission 5 load-bearing geometry

Both Artillery `Cross1` footprints must include `(3,7)`, Gunner `(3,8) -> (2,7)` and Vanguard `(4,7) -> (3,5)` must remain legal public movement paths, and Controller `(3,6) -> (3,7)` must remain a legal push. The real `begin_round` + public movement/displacement regression must not be replaced with direct position mutation.

## Manual validation

Mission 4:

- Bulwark visually reads heavier than the other regular units;
- opening telegraphs remain readable with the Controller push threat;
- both explosive splash and push-into-hazard solutions are discoverable in normal play;
- killing Bulwark ends the mission without escort cleanup;
- session length remains a short tactical encounter.

Mission 5:

- two Artillery telegraphs remain readable alongside Controller/Flanker threats;
- the shared `(3,7)` Artillery footprint is visually understandable;
- vacating the original player cells and pushing Controller into `(3,7)` makes committed crossfire materially useful;
- Controller's own vacated `(3,8)` push threat lands harmlessly;
- the Ram + at least one Mortar hit payoff is apparent in normal play;
- Rapid Break creates useful pressure without making the main objective a hard deadline;
- session length remains a short tactical encounter.

Campaign:

- complete Mission 3 save -> upgrade -> Mission 4 -> results -> upgrade -> Mission 5 -> results -> upgrade -> Mission 6 handoff;
- Continue and restart preserve existing save/upgrades behavior.

## Scope guardrails

No new dependency, crate, objective framework, AI policy framework, generic status/crowd-control system, displacement resistance model, zone of control, aura, pull mechanic, new hazard type, new regular enemy beyond Bulwark/Controller, boss behavior, branching route, mission select, difficulty mode, save migration, new VN art, new glTF file, runtime asset pipeline, transactional battle framework, or second PR.

HPA-523 ends with exactly six regular enemy archetypes and a Mission 6 handoff. Boss-specific behavior belongs to HPA-524.