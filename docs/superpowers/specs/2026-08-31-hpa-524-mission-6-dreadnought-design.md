# HPA-524 Mission 6 Dreadnought Design

## Outcome

Ship Mission 6 as the first boss encounter while keeping the boss on the existing single-cell enemy/combat path. The fight should feel different because the Dreadnought commits large area attacks and switches to a stronger close-pressure salvo at half HP, not because Scorpius gains a boss engine.

Keep this as one HPA-524 PR. Reuse the current deterministic battle state, locked enemy intents, `EliminateTarget`, authored mission openings, campaign save/progression flow, Bevy UI, and checked-in glTF. Do not add a phase scripting system, boss component hierarchy, generic threshold registry, new objective shape, status framework, second battle runtime, or save migration.

## Why this is the next slice

HPA-523 is complete and was HPA-524's only blocker. HPA-524 blocks HPA-386, so Mission 6 is the next unblocked roadmap item.

The current code already supplies the important seams:

- every enemy is a normal `UnitState` on one grid cell;
- enemy movement and target selection already route through `unit_weapon`, while committed attacks store a complete immutable `AttackProfile` and footprint;
- `PrimaryObjective::EliminateTarget` already wins immediately when the authored target is knocked out even if escorts survive;
- regular enemies, hazards, collisions, friendly fire through locked footprints, and one-cell push are already proven;
- missions own board geometry, deployments, openings, dialogue, rewards, and their `MissionDefinition`;
- Mission 5 already advances persisted campaign state to `MissionId::Six`;
- campaign UI decides authored-vs-handoff from `mission_definition`, except `Continue` still explicitly treats Six as the old terminal handoff;
- the checked-in `assets/models/mission_one.gltf` already carries all unit scenes and can accept one more scene without a new asset pipeline.

## Approaches considered

### A — One concrete Dreadnought archetype with a derived half-HP weapon switch — selected

Add `UnitArchetype::Dreadnought`. Give the Mission 6 boss two ordinary enemy weapons in a fixed order. For this archetype only, `unit_weapon` selects slot 0 above half HP and slot 1 at or below half HP.

The threshold is derived from the authored unit itself:

```rust
unit.hp * 2 <= unit.stats.max_hp
```

Mission 6 authors 40 max HP, so the explicit threshold is 20 HP. No phase field is stored. Because the current MVP has no healing, the threshold can only be crossed once. The selected weapon also drives the existing attack-band movement, so the boss naturally changes pressure without another AI layer.

This is the smallest implementation that satisfies the ticket while leaving Mission 7 as the second concrete consumer that may justify a shared threshold seam.

### B — Add generic `BossBehavior` / `ThresholdPhase` data to `UnitState` — rejected for now

A reusable threshold table, phase enum, per-phase movement policy, and serialized boss metadata would make Mission 7 easier to configure, but HPA-524 has only one concrete consumer. Generalize after the final boss proves which parts are actually shared.

### C — Parallel boss runtime or scripted encounter phases — rejected

A boss controller, encounter script, detachable parts, invulnerability transitions, multi-tile occupancy, or callback registry would duplicate battle lifecycle rules and directly violate the ticket's scope.

## Closed domain change

### Dreadnought archetype

Extend the closed unit archetype enum by one concrete enemy:

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

`Dreadnought` is still `Faction::Enemy`, occupies one cell, uses normal HP/Armor/Move/Accuracy/Evasion, takes normal weapon/environment damage, can be knocked out normally, and remains pushable through the existing displacement rule.

Do not add displacement resistance in HPA-524. The boss should remain vulnerable to existing tactical tools unless playtesting proves that authored geometry/HP tuning cannot keep the fight intact. If Mission 7 later needs explicit resistance, that is the second point to evaluate a small shared seam.

### Active enemy weapon selection

Keep the current `unit_weapon` helper as the only selector used by enemy movement. Extend it locally:

```text
Dreadnought above 50% HP -> weapon slot 0
Dreadnought at/below 50% HP -> weapon slot 1
all other enemies -> weapon slot 0
```

Change `build_intent` to call this same helper instead of reading `attacker.weapons.first()` directly. That gives movement and intent commitment one source of truth without introducing a policy object.

If a Dreadnought does not own the required slot, keep the current programmer/data-error behavior (`InvalidTarget`/missing weapon) rather than silently falling back.

### Locked threshold semantics

The threshold never mutates an already-committed intent.

Example:

```text
Round N planning: boss at 21 HP -> Graviton Salvo is committed
Player phase: boss takes 2 damage -> 19 HP
Round N resolution: the committed Graviton Salvo still resolves unchanged
Round N+1 planning: current HP is <= 20 -> Overload Salvo is selected and committed
```

This is important: an intent already contains its weapon ID, damage profile, hit/crit values, origin, footprint, intended occupant, and preview. HPA-524 must preserve that existing immutable contract rather than retroactively upgrading a telegraph after the player has acted around it.

No `BossPhaseChanged` event is required. The player-visible transition is the next committed weapon/telegraph changing name, range band, expected damage, and crit chance through the existing HUD.

### Initiative and movement

Dreadnought initiative is **40**, before Controller 35 and every regular enemy. This makes the boss's committed area attack the first geometry the player must account for during resolution.

For later-round movement, group Dreadnought with the existing attack-band enemies:

```text
Rifleman | Striker | Bulwark | Dreadnought -> attack_band_destination
```

Because `attack_band_destination` receives the active weapon, crossing the threshold also changes desired range from long pressure to close pressure without a second movement implementation.

Keep all enemy movement/initiative matches exhaustive.

## Boss 1 — Dreadnought

Mission 6 owns the boss factory and weapon constants locally in `mission_six.rs`. Do not move boss authoring into a generic shared boss module before Mission 7 exists.

Locked values:

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

The stronger phase is intentionally just a different normal weapon: shorter minimum range, higher damage, higher crit chance. The existing telegraph already shows the new weapon and expected outcome.

## Mission 6 — Break the Dreadnought

### Product intent

The player should read the boss's committed Cross1 as both a major threat and a weapon that can be redirected onto escorts. The fight then tightens at half HP when the Dreadnought switches from range 3–6 to range 1–4 and starts closing distance under the same attack-band planner.

### Board

Use a 9×9 board so the existing camera/readability stays consistent with Missions 4–5.

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

Opening legality is intentional:

- Dreadnought `(4,2)` to Vanguard `(4,7)` is range 5 for Graviton Salvo;
- Bulwark `(1,7)` to Vanguard `(4,7)` is range 3;
- Controller `(6,7)` to Vanguard `(4,7)` is range 2 and row-aligned;
- Rifleman `(6,6)` to Interceptor `(5,8)` is range 3.

Reuse `assert_opening_plan_is_legal`; mission-specific tests still pin the exact rows.

### Opening manipulation line

On the first player phase, the Dreadnought has committed a Cross1 centered on Vanguard `(4,7)`. The footprint contains `(5,7)`.

The authored player line is:

1. move Vanguard from `(4,7)` to `(4,5)`, vacating the Dreadnought and Controller committed target;
2. move Interceptor from `(5,8)` to `(7,7)`;
3. use the existing Vector Pulse geometry on Controller at `(6,7)`, pushing it left to `(5,7)`;
4. resolve the unchanged enemy intents.

The Dreadnought's already-locked Cross1 can then hit Controller at `(5,7)` without retargeting. Controller's own committed `(4,7)` push lands on the now-empty original Vanguard cell. The boss attack therefore becomes the main geometry payoff rather than another damage-race weapon.

Baseline damage reinforces the line without adding special friendly-fire rules:

```text
Controller HP 9 / Armor 1
Vector Pulse normal damage at weapon level 0: 4 - 1 = 3
Graviton Salvo normal damage against Controller: 8 - 1 = 7
```

The exact result remains subject to the existing hit RNG. Tests pin the committed footprint, legal public movement/push geometry, and the boss attack roll targeting Controller; they do not require a guaranteed hit or invent boss-friendly-fire exceptions.

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

Do not add a primary turn limit. Four enemies plus the 40-HP target already supply enough pressure; use manual validation to tune HP/damage before adding another clock.

The target objective means killing the Dreadnought ends the mission immediately even if escorts live, matching HPA-524's boss-focus intent.

### Story

Reuse existing VN assets only: `vn/relay_nine_bg.png`, `vn/control_neutral.png`, `vn/control_alert.png`, and `vn/vanguard_neutral.png`.

Pre-mission:

1. Control: “A Dreadnought is anchoring the line. Its main battery commits before we move.”
2. Vanguard: “Then the escorts are ammunition.”
3. Control: “Exactly. Below half integrity the battery overloads and the Dreadnought will close in.”

Aftermath:

1. Vanguard: “Dreadnought down. Their line is collapsing.”
2. Control: “One command unit remains. Mission 7 is the final push.”

## Campaign handoff

Extend `MissionId` from One–Six to One–Seven.

```text
One–Six -> authored definitions
Seven   -> terminal HPA-524 handoff; `mission_definition(Seven) == None`
```

`MISSION_SIX_DEFINITION` unlocks Seven. Existing `CampaignState::complete_mission` remains unchanged because it already copies `definition.unlocks` into persisted `next_mission` exactly once.

Update campaign UI routing:

```text
Continue One -> PreMissionStory
Continue Two–Six -> Upgrade
Continue Seven -> NextMission
Proceed -> use existing mission_definition(next_mission).is_some() check
```

No new `GameScreen`, mission-select state, save field, or migration layer is added. Pre-release saves may break when `MissionId::Seven` is added.

## Presentation

### Boss telegraph

Do not add a boss-only HUD or phase banner. Existing intent UI already presents:

- weapon name;
- committed footprint;
- intended occupant;
- expected damage;
- hit chance.

The switch from Graviton Salvo to Overload Salvo is therefore visible on the same UI path as every regular intent.

Existing target-objective HUD continues to render Dreadnought HP. No new objective presentation type is needed.

### Dreadnought visual

Append one distinct scene to the existing checked-in glTF.

```text
Scene index 13: Dreadnought
Root node 70
Part nodes 71–76
Mesh/material index 13
Root authored scale 1.12
Material: Dreadnought Crimson
Base color: [0.55, 0.08, 0.12, 1.0]
```

Final asset counts:

```text
14 scenes
77 nodes
14 meshes
14 materials
1 embedded buffer
```

Set `MISSION_ONE_SCENE_COUNT` to 14 and map `UnitArchetype::Dreadnought -> 13` in `scene_index`.

No new glTF file, texture, animation, runtime asset generator, child under-ring, or inverse-scale compensation.

## Testing

Focused deterministic coverage must prove:

1. Dreadnought uses weapon slot 0 at 21 HP and slot 1 at 20 HP; further damage keeps slot 1, so the one-way threshold does not oscillate.
2. A Graviton Salvo committed above half HP remains Graviton Salvo if the boss crosses to 20 HP during the player phase; a newly built next-round intent uses Overload Salvo.
3. Dreadnought remains a normal push target under `resolve_push`.
4. Mission 6 board, roster, opening rows, objective, rewards, and exact boss stats/weapons match the authored values.
5. The first-round real movement/push line puts Controller on `(5,7)` inside the already-committed Dreadnought footprint and leaves the old Vanguard target empty.
6. Knocking out Dreadnought wins immediately with escorts alive.
7. Completing Mission 6 advances once to Seven, rewards 800/250 as appropriate, persists the state, preserves upgrades, and `Continue` routes Six as authored but Seven as the terminal handoff.
8. The glTF contains the expected Dreadnought scene/root/mesh/material and remains one buffer.
9. Existing Missions 1–5 and the full campaign/save/presentation suites remain green.

## Manual validation

Record HPA-524 evidence in `docs/validation/hpa-524.md`:

- start Mission 6 from the real campaign flow after Mission 5;
- confirm Graviton Salvo telegraph is readable and can be redirected onto an escort;
- cross from 21+ HP to 20-or-less during a player phase and confirm the current telegraph does not change retroactively;
- confirm the next planning pass shows Overload Salvo with the stronger/closer pressure;
- confirm pushing the boss still works and does not break intent resolution;
- defeat Dreadnought while at least one escort survives and confirm immediate victory;
- complete aftermath/reward/upgrade flow, return to title, Continue, and confirm the persisted Mission 7 handoff;
- note approximate encounter length and tune authored HP/damage only if the fight is clearly too short/long.

## Scope guardrails

No multi-tile boss, boss parts, invulnerability, cinematic battle scene, unique boss runtime, threshold registry, phase scripting, generic behavior policies, displacement immunity, new status effect, new objective variant, new optional objective, new hazard/prop type, new progression track, new save field, save migration, new crate/dependency, second boss, Mission 7 content, new VN art, second glTF, or asset pipeline.

Mission 7 is deliberately the next and second threshold consumer. If it needs a different threshold shape, generalize only the common behavior proven by both encounters.