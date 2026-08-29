# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius from the completed Mission 1 campaign loop to three continuously playable missions, adding a Gunner-defense encounter, a real Courier chase with reachable extraction, and a distinct Flanker enemy without introducing a generic objective or AI framework.

**Architecture:** `BattleState` gains one closed `MissionRules` row covering only the objective/opening shapes Missions 1–3 use. `mission` remains the typed authoring layer with a small shared regular-enemy catalog and separate Mission 2/3 modules. Flanker stays explicit in the existing deterministic planner; the existing campaign/save/UI composition remains intact. The checked-in glTF is extended by one authored scene rather than adding runtime scale/marker compensation.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, existing headless Rust tests, existing checked-in `assets/models/mission_one.gltf` and VN PNGs.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One Linear ticket (`HPA-637`) = one implementation PR; continue implementation on this draft PR.
- Keep dependency direction `presentation -> mission -> domain`; `src/domain/` must not import Bevy or campaign/presentation types.
- Keep one application crate and `bevy = "0.19"`; add no dependency.
- Preserve committed-intent semantics: player movement never retargets an already committed enemy intent.
- Keep mission content typed in Rust; add no RON/JSON/scripting/content framework.
- Add no objective callback/trait/registry, behavior tree, utility-AI framework, pathfinding dependency, stealth, teleportation, new initiative system, status framework, new playable unit, deployment selection, mission select, branching, difficulty, boss, or new hazard type.
- Mission 2 protects the existing Gunner; do not add a neutral faction/objective-unit role.
- Mission 2 wins when Gunner survives through the real Round-3 enemy-resolution boundary **or** when all attackers are eliminated while Gunner is alive.
- Mission 3 Courier starts `(0,6)`, extraction is `(8,0)`, and the deadline is Round 5. Three move-4 passes cannot extract before player Round 4; the fourth can extract; Round 5 is only the blocked/stalled backstop.
- Optional objectives affect credits only and never gate progression.
- Extend the existing checked-in glTF with one Flanker scene; add no new asset file or generation pipeline.
- Keep save state to `next_mission`, credits, and upgrades; add no migration/version compatibility code.
- All automated tests stay headless.
- Final gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release`.

---

## Task 1 — Closed mission rules + generic bonus + named round boundary

**Files:** `src/domain/model.rs`, `src/domain/battle.rs`, `src/campaign/progression.rs`, `src/mission/mission_one.rs`, `src/presentation/ui.rs`, affected tests.

**Produces:** `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `MissionRules`, `BattleState::rules()`, private `completed_enemy_round(round)`; generic `optional_complete` result/progress.

- [ ] Write red focused tests: protect enemy-clear win, protected-target KO fail, intercept target-KO win/escort-clear non-win.
- [ ] Add a red helper-semantic test: `round=3` in Player phase is **not** a completed Round3 boundary; the same numeric round in EnemyPlanning is.
- [ ] Add exact closed types from the spec and change `BattleState::new(..., rules, seed)`.
- [ ] Add:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

- [ ] Implement primary outcome:

```rust
match self.rules.primary {
    PrimaryObjective::EliminateAllEnemies => (!any_living_enemy).then_some(true),
    PrimaryObjective::ProtectThroughRound { target, round } => {
        let target = self.units.get(&target).expect("authored protected target must exist");
        if target.is_knocked_out() { Some(false) }
        else if !any_living_enemy || self.completed_enemy_round(round) { Some(true) }
        else { None }
    }
    PrimaryObjective::InterceptBeforeEscape { target, escape, deadline_round } => {
        let target = self.units.get(&target).expect("authored interception target must exist");
        if target.is_knocked_out() { Some(true) }
        else if target.position == escape || self.completed_enemy_round(deadline_round) { Some(false) }
        else { None }
    }
}
```

- [ ] Keep Turnabout trigger special; implement terminal-only half-HP / victory-by-round bonus checks with one `optional_complete` bit. Newly earned bonus event precedes `MissionCompleted`.
- [ ] Rename active `turnabout_complete` fields to `optional_complete`; update campaign reward logic; leave historical docs untouched.
- [ ] Run `cargo fmt --check`, `cargo test --lib domain::battle::`, `cargo test --all-targets`.
- [ ] Commit `feat: add authored mission objective rules`.

---

## Task 2 — Authored openings + shared enemies + Flanker planner/initiative

**Files:** create `src/mission/enemies.rs`; modify mission mod/mission_one, domain model/enemy, presentation interaction/battlefield exhaustive matches.

- [ ] Strengthen existing Mission1 opening test with exact intended occupants before refactor.
- [ ] Create shared enemy factories:

```text
Rifleman  HP9  Armor1 Move2 Acc72 Eva5  Service Rifle
Striker   HP12 Armor2 Move2 Acc78 Eva10 Shock Claw
Artillery HP10 Armor1 Move1 Acc90 Eva0  Siege Mortar
Flanker   HP8  Armor0 Move4 Acc82 Eva30 Skirmish Carbine
```

Skirmish Carbine: range1–2, Single, damage4, hit+5, crit10, EN0, no push/counter.

- [ ] Move Mission1 opening to four `EnemyOpening` rows exactly matching current behavior.
- [ ] Delete Mission1 archetype/x-position opening movement + `opening_target` hardcoding.
- [ ] Write planner tests:
  - protect Flanker moves into legal band around protected Gunner;
  - protect Flanker intent prefers Gunner when legal;
  - Courier reduces distance to `(8,0)`;
  - non-objective Flanker moves using attack-band fallback;
  - equal-distance tie chooses more open orthogonal neighbors.
- [ ] Extract local `choose_attack_band_destination(...)` and reuse for Rifleman/Striker/fallback Flanker. No policy objects.
- [ ] Use protect sort `(band distance, Manhattan, Reverse(open neighbors), y, x)` and Courier sort `(Manhattan to escape, Reverse(open neighbors), y, x)`.
- [ ] Replace positional initiative hack with:

```rust
Striker => 30,
Flanker => 25,
Rifleman => 20,
Artillery => 10,
_ => 0,
```

Add exact initiative test; Mission1 order remains pinned by existing test.
- [ ] Run domain enemy + Mission1 + all-target tests.
- [ ] Commit `feat: add authored enemy openings and flanker`.

---

## Task 3 — Mission 2 + authoring validation + MissionId One–Four once

**Files:** create `src/mission/mission_two.rs`; modify mission mod, campaign UI, enemy tests.

- [ ] Authoring tests pin 9×9 board, deployment V `(3,7)` / G `(4,6)` / I `(5,7)`, blockers `(3,3),(5,3),(2,6),(6,6)`, hazards `(1,5),(7,5)`, explosive `(6,4)` HP4, enemy IDs21–24, protect rule/bonus.
- [ ] Validate protected Gunner exists; every opening enemy/target reference exists with correct faction; every opening destination is in bounds/non-blocking.
- [ ] Pin Gunner HP1 upgrade projection to max HP15.
- [ ] Opening rows:

```text
Rifleman -> (2,4), target Vanguard
Striker -> (4,5), target Gunner
Artillery -> (4,0), target Gunner
Flanker -> (5,5), target Interceptor
```

- [ ] Definition/copy:

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
400 + 100; unlock Three
```

- [ ] Lifecycle test A: begin Round1, KO all four enemies; final KO immediately produces victory/MissionCompleted.
- [ ] Lifecycle test B: build with Gunner HP3/Armor3, Guard all living players, keep attackers alive; Round1 resolve -> player Round2/no result; Round2 -> player Round3/no result; Round3 resolve -> victory.
- [ ] Gunner KO fail; exact half-HP bonus true; one below half bonus false; event ordering pinned.
- [ ] Add `MissionId { One, Two, Three, Four }` **now**. In this task One/Two authored; Three/Four handoffs. Add `number()`.
- [ ] Final Continue routing already becomes One→story, Two/Three→Upgrade, Four→handoff. Upgrade Proceed checks whether next mission is authored.
- [ ] Run Mission2/domain/all-target tests.
- [ ] Commit `feat: add mission 2 gunner defense`.

---

## Task 4 — Mission 3 + live extraction + Round5 backstop + push loss

**Files:** create `src/mission/mission_three.rs`; modify mission mod/campaign UI/progression tests/campaign persistence.

- [ ] Authoring tests pin 9×9 board, players `(4,7)/(3,8)/(5,8)`, blockers `(4,3),(4,4),(4,5)`, hazard `(2,5)`, explosive `(6,3)` HP4, Courier31/Rifleman32/Striker33.
- [ ] Pin Courier Flanker at `(0,6)`, extraction `(8,0)`, deadline5, Manhattan14.
- [ ] Validate opening references and assert extraction is in-bounds, non-blocking, non-hazard, no live explosive.
- [ ] Definition/copy:

```text
Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
500 + 150; unlock Four
```

- [ ] Focused outcomes: escort-clear non-win; Courier KO with escort alive win; Round2 bonus yes/Round3 no; Courier already on exit fail; EnemyPlanning round5 fail.
- [ ] Timing tests use HP3/Armor3 on all players + Guard helper.
- [ ] **Round4 availability test:** remove escorts; resolve Rounds1–3; assert Player rounds2/3/4 with no result; Courier not on `(8,0)` at player Round4. Do not assert exact distance.
- [ ] **Open extraction test:** from equivalent fixture resolve player Round4; assert Courier reaches `(8,0)` during move#4 and mission fails by extraction.
- [ ] **Blocked-exit test:** place durable Interceptor on `(8,0)`; after player Round4 resolve assert Player Round5/no result/Courier not exit; resolve Round5 and assert deadline fail before another Courier move (position unchanged).
- [ ] **Player-caused push test:** place Vanguard `(6,0)`, Courier `(7,0)`, `resolve_push`; assert Courier `(8,0)` and immediate MissionFailed.
- [ ] Author `mission_definition(Three)`; Four remains handoff. No enum churn.
- [ ] Progression: complete One/Two/Three no bonus -> Four + 1200 credits; bonus affects credits only. Save/load Four + nonzero upgrades exactly.
- [ ] Run Mission3/progression/persistence/all-target tests.
- [ ] Commit `feat: add mission 3 courier interception`.

---

## Task 5 — Distinct Flanker glTF scene + objective-generic presentation

**Files:** existing glTF, presentation assets/battlefield/ui/campaign UI, relevant tests.

- [ ] Red JSON asset test parses glTF and asserts 11 scenes, scene10 `Flanker`, nodes `[49..55]`, mesh10/material10 `Flanker Magenta`.
- [ ] Append scene10 with nodes:

```text
49 Left Leg      [-0.16,0.18, 0.00] scale [0.12,0.36,0.16]
50 Right Leg     [ 0.16,0.18, 0.00] scale [0.12,0.36,0.16]
51 Torso         [ 0.00,0.62, 0.00] scale [0.36,0.42,0.28]
52 Head          [ 0.00,0.95, 0.00] scale [0.20,0.20,0.20]
53 Left Fin      [-0.42,0.67,-0.10] scale [0.42,0.08,0.28]
54 Right Fin     [ 0.42,0.67,-0.10] scale [0.42,0.08,0.28]
55 Rear Thruster [ 0.00,0.52,-0.34] scale [0.20,0.16,0.34]
```

All use mesh10. Mesh10 uses existing POSITION0/NORMAL1/indices2, material10. Material10 base color `[0.78,0.08,0.46,1]`, metallic.25, roughness.62, emissive `[0.08,0,0.04]`. Buffer/accessors unchanged.
- [ ] Set `MISSION_ONE_SCENE_COUNT=11`; `scene_index(Flanker)=10`; keep root scale0.72. No `unit_scale`, child under-ring, inverse-scale math.
- [ ] HUD tests: M2 full protect-or-clear copy + Round1/3 + Gunner HP; M3 Round1/5 + cells-from-extraction; bonus labels.
- [ ] Result/event/reward copy becomes objective-generic (`BONUS OBJECTIVE COMPLETE`, `Bonus +...`).
- [ ] Spawn extraction ring at rule escape `(8,0)` with existing white ring material.
- [ ] Genericize touched root name to `Mission Presentation`.
- [ ] Run UI/campaign_flow/presentation/all-target tests.
- [ ] Commit `feat: add flanker presentation and mission objective UI`.

---

## Task 6 — Campaign/restart/save integration through Mission 3

**Files:** `tests/presentation_app.rs`, `tests/campaign_flow.rs`, `tests/campaign_persistence.rs`; source only if tests reveal a real assumption.

- [ ] Mission2 entry with Gunner HP1 -> ActiveMission Two, protect rules, round1, maxHP15.
- [ ] Mission3 entry/restart -> ActiveMission Three, CourierHP8, escape `(8,0)`, deadline5; restart remains definition-driven.
- [ ] Routing assertions:

```text
Continue One -> Story
Continue Two -> Upgrade
Continue Three -> Upgrade
Continue Four -> Handoff
Proceed Two -> Story
Proceed Three -> Story
Proceed Four -> Handoff
```

- [ ] Save-backed continuity: M1 no bonus, buy Vanguard HP1; M2 no bonus, buy Gunner HP1; M3 no bonus; reload -> Four, 800 credits, both upgrades.
- [ ] Run three integration suites + all-target tests.
- [ ] `git add` only the three test files. If a source fix is required, stage that exact path separately; do not speculatively stage `app.rs`/`interaction.rs`.
- [ ] Commit `test: cover campaign progression through mission 3`.

---

## Task 7 — Docs, manual feel validation, final evidence

- [ ] README: three-mission flow, rewards, Continue, M2 protect-or-clear, M3 extraction/Round5, distinct Flanker, unchanged controls.
- [ ] CLAUDE.md: current campaign/mission rules, `completed_enemy_round`, shared enemies, explicit Flanker planner/fallback, fixed initiative values, glTF scene10, committed-intent invariant.
- [ ] Run fmt, strict Clippy, all-target tests, release build before manual play.
- [ ] Manual M2: competing threats, meaningful reactions/Aegis, Gunner KO fail, immediate clear win, Round3 survival win with enemy alive, bonus achieved/missed.
- [ ] Manual M3: magenta Flanker, extraction ring, player Round4 exists, open extraction after Round4, blocked exit Round5 backstop, Courier-only victory, escort-clear non-victory, early bonus.
- [ ] Manual save/Continue/upgrades after M1/M2 and M4 handoff after M3.
- [ ] `docs/validation/hpa-637.md`: exact head SHA, gate/test counts, named M2/M3 lifecycle tests, push regression, glTF scene evidence, manual observations, save continuity, short-session verdict. No placeholders.
- [ ] Re-run full gates and commit `docs: validate HPA-637 missions 2 and 3`.

---

## Final PR Gate

- [ ] One PR, HPA-637 scope only; no new dependency/framework/runtime asset pipeline.
- [ ] Mission1 opening regression green.
- [ ] `completed_enemy_round` is the only objective round-boundary predicate.
- [ ] M2: Gunner KO fails; enemy clear wins immediately; three real enemy resolutions also win with enemy alive; competing opening locks exact.
- [ ] M3: 14-step route, player Round4 exists, fourth move extracts on open route, blocked exit reaches Round5 backstop, push-to-exit fails immediately.
- [ ] Courier KO with escorts alive wins; escort clear alone does not.
- [ ] Flanker fallback moves; initiative Striker30/Flanker25/Rifleman20/Artillery10; no positional Rifleman hack.
- [ ] Mission authoring references/extraction legality tested.
- [ ] Flanker glTF scene10, scene count11; no `unit_scale`/under-ring compensation.
- [ ] Extraction ring uses existing white material.
- [ ] M2 HUD n/3; M3 HUD n/5; objective/result/reward copy generic.
- [ ] One→Two→Three→Four save/upgrade flow; base-only 1200 credits.
- [ ] README/CLAUDE/validation current.
- [ ] fmt, strict Clippy, all-target tests, release build pass.

## Self-review

The reviewed changes are all encoded as implementation requirements rather than deferred playtest choices: live extraction with Round5 backstop, immediate M2 clear win, distinct glTF scene, named boundary helper, fixed initiative values, authoring tests, push-to-exit test, one-time MissionId growth, and non-speculative staging. No placeholder or extra framework remains.