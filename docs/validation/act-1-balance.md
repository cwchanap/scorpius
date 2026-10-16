# Act 1 balance pass

Date: 2026-10-16. Requested by Hapadona in place of a human playtest.

## Goal

The seven authored missions are **Act 1**: an introduction where a new player learns the basic
mechanics. The pass tunes for a gentle onboarding curve in which each mission's featured mechanic is
the play that pays off. It does not tune for challenge.

No human played these numbers. Everything below comes from `tests/balance.rs`, a headless
simulation that plays the campaign through the public domain API.

## Method

`tests/balance.rs` plays full campaigns (Mission 1 through Mission 7) with three scripted player
policies:

| Policy | Plays like |
| --- | --- |
| Novice | Attacks the best visible target and fires Focus on its first attack. Rotates Guard, Counter and Evade without reading the telegraphs. Activates the roster in order and never dodges a committed footprint. |
| Reader | Same attack scoring. Also steps out of committed footprints, picks the reaction that minimizes the locked incoming damage, uses Aegis on an exposed ally, and values the collision and hazard damage a push preview shows. |
| Tactician | Reader, plus it values pushing enemies into other enemies' locked fire and into explosive blasts: the "turn their plan against them" hook. |

The bots score options from attack previews and committed intents only, never from the battle RNG.
A campaign retries a lost mission (at most 12 tries) and then buys upgrades cheapest-first in a
fixed order. Seeds are fixed, so every number below can be reproduced exactly.

Each table below comes from 200 campaigns per policy, the harness default:

```bash
cargo test --release --test balance -- --nocapture
```

## Rubric

What "an introduction" means in numbers:

1. **Anyone clears it.** The Novice wins at least 90% of first tries, needs at most 1.5 tries per
   clear, and no run gets stuck.
2. **Reading the telegraphs pays.** Wherever enemies commit real damage, the Reader finishes with
   clearly more HP than the Novice. The intent system is the core mechanic, so ignoring it should
   hurt and reading it should feel safe.
3. **The mechanic shows before the mission ends.** Few wins come in Round 1, so the player sees at
   least one enemy round of the mission's featured idea.
4. **The bosses are the peak.** The Novice's worst HP result comes at the end of the act.

## Baseline findings

Before the tuning, every policy won every mission on the first try with almost no knockouts:

| Mission | Novice HP left | Reader HP left | Round 1 wins (Reader) | What went wrong |
| --- | ---: | ---: | ---: | --- |
| 1 | 81% | 100% | 0% | Ignoring telegraphs cost little. |
| 2 | 79% | 100% | 0% | Same. |
| 3 | 100% | 100% | **100%** | The Courier started inside first-turn reach. Focus + Overcharge, or Anchor Cannon plus a wall collision, killed it before it moved. The chase never happened. |
| 4 | 100% | 100% | **88%** | A Focus Burst Missile detonated both gate explosives, and the 16 HP Gate Bulwark died to the double blast before its escorts acted. |
| 5 | 94% | 100% | 0% | Ignoring the mortar crossfire cost little. |
| 6 | 88% | 100% | 0% | The Dreadnought barely dented a squad that ignored it. |
| 7 | 75% | 94% | 0% | Same for the Regent. |

The Novice's HP loss was small everywhere, so rubric 2 failed on every mission. Missions 3 and 4
failed rubric 3 outright.

## Changes

| Change | Before | After | Why |
| --- | --- | --- | --- |
| Siege Mortar base damage (Artillery) | 6 | 8 | The telegraphed Cross1 blast is the clearest "step out of the footprint" lesson. It now costs a careless player. |
| Shock Claw base damage (Striker) | 7 | 9 | The Striker telegraphs a melee lunge. Guarding or stepping away now matters. |
| Graviton Salvo / Overload Salvo (Dreadnought) | 8 / 10 | 10 / 12 | The first boss should be the moment ignoring intents really hurts. |
| Command Barrage / Rupture Beam (Regent) | 9 / 12 | 11 / 14 | Same for the final boss, which stays the peak of the act. |
| Mission 3 Courier start | (0,6) | (0,2) | One cell beyond every mech's first-turn reach (move plus weapon range), so the intercept is a chase. The race to the exit drops from Manhattan 14 to 10: the Courier now extracts after its third move (end of Round 3) instead of its fourth, so the player gets Rounds 2–3 to catch it instead of a turn-1 kill. The Round 5 deadline stays a backstop for a blocked exit, as before. |
| Mission 4 Gate Bulwark max HP | 16 | 22 | The double detonation softens it (22 → 14) instead of ending the mission before the escorts act. Ramming it into the trench and finishing it stays the solution. The shared Bulwark archetype keeps 16 HP everywhere else. |

Variants tested and rejected:

- **Courier at (0,3).** Still reachable on turn 1.
- **Courier at (1,1).** Too hard: every policy won only 72–74% of first tries.
- **Courier HP 12–14.** Turn-1 kills stayed near 90%.
- **Gate Bulwark at 20 and 24 HP.** At 20 HP, 28% of wins came in Round 1; at 24 HP, 5% did. 22 HP (13–15%) is the smaller change and already meets rubric 3.
- **Boss HP 48–56.** It mostly made the boss fights longer. Raising the boss weapons' damage works on rubric 2 directly, so that was kept instead.

## Results after the tuning

| Mission | Novice first try | Novice tries/clear | Novice HP left | Novice KOs/try | Reader first try | Reader HP left | Round 1 wins (Reader) | Reader rounds | Reader bonus | Tactician bonus |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 70% | 0.1 | 100% | 99% | 0% | 3.9 | 100% | 100% |
| 2 | 90% | 1.1 | 70% | 0.1 | 100% | 100% | 0% | 3.0 | 100% | 100% |
| 3 | 94% | 1.1 | 100% | 0.0 | 99% | 100% | 0% | 2.2 | 77% | 78% |
| 4 | 100% | 1.0 | 100% | 0.0 | 100% | 100% | 15% | 1.9 | 100% | 100% |
| 5 | 100% | 1.0 | 88% | 0.0 | 100% | 100% | 0% | 4.7 | 46% | 58% |
| 6 | 100% | 1.0 | 85% | 0.0 | 100% | 100% | 0% | 3.3 | 28% | 56% |
| 7 | 100% | 1.0 | 62% | 0.2 | 100% | 99% | 0% | 5.7 | 86% | 73% |

Against the rubric:

1. **Anyone clears it.** Yes. The Novice's worst first-try rate is 90% (Mission 2, where it
   sometimes loses the Gunner it must protect) and never needs more than 1.1 tries per clear. No
   run got stuck.
2. **Reading the telegraphs pays.** Yes, on every mission where enemies commit damage. The gap is
   29–37 points of HP on Missions 1, 2 and 7, and 12–15 points on Missions 5 and 6. Missions 3
   and 4 are objective puzzles with little incoming fire, so their lessons are the chase and the
   breach, not survival.
3. **The mechanic shows before the mission ends.** Mission 3 no longer ends in Round 1. Mission 4
   still does 13–15% of the time, down from about 90%.
4. **The bosses are the peak.** Yes: Mission 7 is the Novice's worst result (62% HP, 0.2 KOs per
   try).

The "turn their plan against them" hook now pays off: the Tactician earns Mission 6's Turnabout
bonus 56% of the time against the Reader's 28%, and Mission 5's Rapid Break 58% against 46%.

Full per-policy tables, before and after, are at the end of this file. In them, the four intent
columns say how committed enemy attacks resolved (hit the squad, hit other enemies, hit an empty
footprint, or were canceled by the attacker's knockout). The two enemy-damage columns are the share
of damage enemies took from their own side's fire and from collisions, hazards and explosions. Focus
and Gunner share are the share of enemy damage dealt by Focus attacks and by the Gunner. Credits in
is the mean credit balance on entering the mission.

## Guard rails

`cargo test --all-targets` runs the same 200 campaigns per policy and asserts the rubric, so a
later change that flattens the curve fails a test:

- No campaign run gets stuck, and the bots never issue an illegal command.
- Every policy wins at most 25% of its clears in Round 1.
- The Reader and the Tactician win at least 95% of first tries on every mission.
- The Novice wins at least 85% of first tries, with at most 1.5 tries per clear, on every mission.
- On Missions 1, 2, 5, 6 and 7, the Reader keeps at least 10 points more HP than the Novice.
- Mission 7 is the Novice's lowest-HP mission.
- On Mission 6, the Tactician earns Turnabout at least 15 points more often than the Reader.

The run takes about 9 seconds. The baseline tables break these rails: Round 1 wins on
Missions 3 and 4, the HP gap on Mission 5 (94% against 100%), and the Turnabout margin on Mission 6
(28% against 25%). A smaller sample (`SCORPIUS_BALANCE_RUNS` below 200) prints the report and skips
the rubric.

## Spec values this pass supersedes

- `2026-08-23-hpa-632-*-design.md`: Shock Claw damage 7 and Siege Mortar damage 6 are now 9 and 8.
- `2026-08-28-hpa-637-*-design.md`: the Courier now starts and holds at (0,2), not (0,6). The race
  to (8,0) is Manhattan 10, not 14, so the third move (after player phase 3) extracts instead of
  the fourth.
- `2026-08-30-hpa-523-*-design.md`: the Gate Bulwark has 22 HP, not 16 (`TARGET Gate Bulwark HP
  22/22`). Siege Mortar normal damage against the Controller is 7, not 5.
- `2026-08-31-hpa-524-mission-6-dreadnought-design.md`: Graviton and Overload damage are 10 and 12.
- `2026-09-02-hpa-386-mission-7-mvp-closeout-design.md`: Command Barrage and Rupture Beam damage
  are 11 and 14.

## Caveats

- Bots are not people. The Novice still picks a strong attack every activation, so a real first-time
  player will likely take more damage than it does. These numbers are a floor on difficulty, not a
  prediction.
- The harness plays each mission's authored battle arena. Mission 1's 128×128 regional map beyond
  the arena is not exercised.
- Bonus rates for Swift Intercept (Mission 3) and Final Push (Mission 7) depend on how aggressively
  a player chases. The bots' aggression is a guess.
- A human playtest of Act 1 is still the real test. The rubric above is what to watch for.

## Full tables: before

#### Novice

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.5 | 0% | 88% | 81% | 0.0 | 60% | 0% | 5% | 35% | 6% | 4% | 22% | 34% | 0.0 |
| 2 | 100% | 1.0 | 0 | 2.4 | 0% | 50% | 79% | 0.0 | 53% | 0% | 4% | 43% | 11% | 0% | 21% | 45% | 12.5 |
| 3 | 100% | 1.0 | 0 | 1.0 | 100% | 100% | 100% | 0.0 | — | — | — | — | 0% | 6% | 48% | 48% | 49.0 |
| 4 | 100% | 1.0 | 0 | 1.1 | 92% | 66% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 19% | 52% | 46% | 99.0 |
| 5 | 100% | 1.0 | 0 | 4.8 | 0% | 28% | 94% | 0.0 | 27% | 0% | 41% | 32% | 0% | 0% | 17% | 47% | 66.0 |
| 6 | 100% | 1.0 | 0 | 3.3 | 0% | 12% | 88% | 0.0 | 37% | 1% | 59% | 3% | 1% | 0% | 19% | 42% | 174.0 |
| 7 | 100% | 1.0 | 0 | 5.2 | 0% | 90% | 75% | 0.1 | 33% | 0% | 59% | 8% | 2% | 1% | 12% | 36% | 166.0 |

#### Reader

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.9 | 0% | 100% | 100% | 0.0 | 4% | 11% | 51% | 34% | 6% | 11% | 22% | 42% | 0.0 |
| 2 | 100% | 1.0 | 0 | 3.0 | 0% | 100% | 100% | 0.0 | 15% | 0% | 42% | 43% | 9% | 7% | 24% | 34% | 0.5 |
| 3 | 100% | 1.0 | 0 | 1.0 | 100% | 100% | 100% | 0.0 | — | — | — | — | 0% | 0% | 100% | 100% | 99.5 |
| 4 | 100% | 1.0 | 0 | 1.1 | 88% | 100% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 50% | 2% | 16% | 149.5 |
| 5 | 100% | 1.0 | 0 | 4.7 | 0% | 46% | 100% | 0.0 | 0% | 1% | 65% | 34% | 0% | 3% | 17% | 49% | 99.5 |
| 6 | 100% | 1.0 | 0 | 3.3 | 0% | 25% | 100% | 0.0 | 1% | 0% | 93% | 6% | 0% | 2% | 20% | 44% | 110.5 |
| 7 | 100% | 1.0 | 0 | 4.9 | 0% | 94% | 94% | 0.0 | 11% | 2% | 76% | 11% | 1% | 5% | 15% | 42% | 91.0 |

#### Tactician

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.9 | 0% | 100% | 100% | 0.0 | 4% | 11% | 51% | 34% | 6% | 11% | 22% | 42% | 0.0 |
| 2 | 100% | 1.0 | 0 | 3.0 | 0% | 100% | 100% | 0.0 | 15% | 0% | 42% | 43% | 9% | 7% | 24% | 34% | 0.5 |
| 3 | 100% | 1.0 | 0 | 1.0 | 100% | 100% | 100% | 0.0 | — | — | — | — | 0% | 0% | 100% | 100% | 99.5 |
| 4 | 100% | 1.0 | 0 | 1.1 | 88% | 100% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 50% | 2% | 16% | 149.5 |
| 5 | 100% | 1.0 | 0 | 4.5 | 0% | 56% | 100% | 0.0 | 0% | 3% | 63% | 34% | 1% | 3% | 17% | 48% | 99.5 |
| 6 | 100% | 1.0 | 0 | 3.3 | 0% | 28% | 100% | 0.0 | 1% | 1% | 91% | 7% | 1% | 1% | 20% | 43% | 89.5 |
| 7 | 100% | 1.0 | 0 | 5.1 | 0% | 90% | 93% | 0.0 | 13% | 3% | 72% | 12% | 2% | 6% | 14% | 41% | 89.5 |

## Full tables: after

#### Novice

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.5 | 0% | 88% | 70% | 0.1 | 61% | 1% | 4% | 35% | 7% | 4% | 22% | 33% | 0.0 |
| 2 | 90% | 1.1 | 0 | 2.3 | 0% | 34% | 70% | 0.1 | 52% | 0% | 3% | 44% | 15% | 0% | 21% | 43% | 12.5 |
| 3 | 94% | 1.1 | 0 | 2.1 | 0% | 88% | 100% | 0.0 | 0% | 0% | 61% | 39% | 0% | 1% | 35% | 49% | 37.0 |
| 4 | 100% | 1.0 | 0 | 1.9 | 13% | 100% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 36% | 47% | 13% | 85.0 |
| 5 | 100% | 1.0 | 0 | 4.8 | 0% | 28% | 88% | 0.0 | 27% | 0% | 41% | 32% | 0% | 0% | 17% | 47% | 43.0 |
| 6 | 100% | 1.0 | 0 | 3.3 | 0% | 12% | 85% | 0.0 | 36% | 1% | 60% | 3% | 1% | 0% | 19% | 42% | 155.0 |
| 7 | 100% | 1.0 | 0 | 5.1 | 0% | 95% | 62% | 0.2 | 32% | 0% | 60% | 8% | 3% | 1% | 12% | 35% | 161.0 |

#### Reader

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.9 | 0% | 100% | 99% | 0.0 | 3% | 11% | 52% | 34% | 7% | 12% | 22% | 42% | 0.0 |
| 2 | 100% | 1.0 | 0 | 3.0 | 0% | 100% | 100% | 0.0 | 0% | 16% | 51% | 33% | 13% | 8% | 25% | 31% | 0.0 |
| 3 | 99% | 1.0 | 0 | 2.2 | 0% | 77% | 100% | 0.0 | 0% | 0% | 46% | 54% | 0% | 0% | 33% | 56% | 100.0 |
| 4 | 100% | 1.0 | 0 | 1.9 | 15% | 100% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 46% | 14% | 24% | 115.5 |
| 5 | 100% | 1.0 | 0 | 4.7 | 0% | 46% | 100% | 0.0 | 0% | 1% | 65% | 34% | 0% | 2% | 17% | 49% | 111.5 |
| 6 | 100% | 1.0 | 0 | 3.3 | 0% | 28% | 100% | 0.0 | 1% | 0% | 95% | 4% | 0% | 2% | 20% | 44% | 115.5 |
| 7 | 100% | 1.0 | 0 | 5.7 | 0% | 86% | 99% | 0.0 | 1% | 0% | 80% | 19% | 0% | 5% | 13% | 40% | 112.2 |

#### Tactician

| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100% | 1.0 | 0 | 3.9 | 0% | 100% | 99% | 0.0 | 3% | 11% | 52% | 34% | 7% | 12% | 22% | 42% | 0.0 |
| 2 | 100% | 1.0 | 0 | 3.0 | 0% | 100% | 100% | 0.0 | 0% | 16% | 51% | 33% | 13% | 8% | 25% | 31% | 0.0 |
| 3 | 99% | 1.0 | 0 | 2.2 | 0% | 78% | 100% | 0.0 | 0% | 3% | 44% | 54% | 2% | 0% | 32% | 56% | 100.0 |
| 4 | 100% | 1.0 | 0 | 1.9 | 15% | 100% | 100% | 0.0 | 0% | 0% | 100% | 0% | 0% | 46% | 14% | 24% | 117.8 |
| 5 | 100% | 1.0 | 0 | 4.5 | 0% | 58% | 100% | 0.0 | 1% | 3% | 63% | 33% | 1% | 1% | 18% | 47% | 110.8 |
| 6 | 100% | 1.0 | 0 | 3.4 | 0% | 56% | 100% | 0.0 | 0% | 9% | 79% | 12% | 7% | 1% | 19% | 41% | 106.8 |
| 7 | 100% | 1.0 | 0 | 6.0 | 0% | 73% | 99% | 0.0 | 0% | 2% | 77% | 21% | 1% | 4% | 12% | 41% | 85.5 |
