# Scorpius

Scorpius is a retained desktop turn-based tactics game built with Rust 2024 and Bevy 0.19. HPA-635 wraps the validated Mission 1 combat slice in one linear campaign loop, HPA-637 authors Missions 2–3 plus the Flanker enemy, and HPA-523 authors Missions 4–5 plus the Bulwark and Controller enemies so the loop runs:

**Title → pre-mission VN → briefing → Mission 1 → result/reward → aftermath → mech upgrades → Mission 2 → … → Mission 5 → … → Mission 6 handoff.**

## Run

Install a stable Rust toolchain that supports edition 2024, then run:

```bash
cargo run
```

The game starts at the **Title** screen.

- **NEW GAME** starts a fresh campaign at Mission 1 and immediately overwrites any existing pre-release progress.
- **CONTINUE** loads the saved campaign. It is disabled when no save exists (and shows the same error shape for an unreadable/corrupted one). Progress at Mission 1 resumes at the pre-mission story; progress at Missions 2–5 resumes at the upgrade screen; progress at the Mission 6 handoff resumes at the handoff screen. It never replays a reward: mission completion is granted exactly once, and a repeated victory Continue fails the advancement guard instead of paying twice.

### Save data

One concrete JSON save per platform; no save migrations or version compatibility, no multiple slots, and no mid-battle resume, checkpoints, or rewind:

- macOS: `~/Library/Application Support/Scorpius/campaign.json`
- Windows: `%APPDATA%\Scorpius\campaign.json`
- Linux: `$XDG_DATA_HOME/scorpius/campaign.json` (fallback `~/.local/share/scorpius/campaign.json`)

The save persists only `next_mission`, credits, and the fixed squad's upgrade levels.

## Campaign progression

**Mission 1 — Turnabout at Relay Nine** pays a 300-credit base reward on victory, plus 100 bonus credits when the optional **Turnabout** objective (damage an enemy with enemy fire, collision, hazard, or explosion) is complete.

After victory, the **Aftermath** screen shows the exact persisted receipt (base, bonus, total, remaining credits), and credits buy permanent mech upgrades on the upgrade screen:

| Track | Cost per level (1/2/3) | Effect per level |
| --- | --- | --- |
| HP | 200 / 400 / 600 | +3 max HP |
| Armor | 200 / 400 / 600 | +1 armor |
| Mobility | 200 / 400 / 600 | +5 evasion |
| Weapon | 200 / 400 / 600 | +1 base damage to the mech's three weapons |

Purchases are validated before mutation: unaffordable or already-maxed purchases are no-ops and never write the save. A valid purchase is serialized to the save file before the in-memory session state is replaced, so a failed write never advances in-memory state ahead of disk; a crash mid-write surfaces as a save-file error on the next load. Upgrade effects apply the next time the mission is built.

The final **MISSION 6 UNLOCKED** screen is a saved handoff state only; Mission 6 content is not authored in this build.

## Mission flow

**Mission 1 — Turnabout at Relay Nine.** The primary objective is to eliminate all four enemies. The optional **Turnabout** objective is achieved when an enemy takes damage from a committed enemy attack, collision, hazard, or explosion. The mission fails when all three player mechs are knocked out.

**Mission 2 — Hold Relay Nine** (400 base + 100 bonus credits, unlocks Mission 3). Protect the Gunner: the mission fails if the Gunner is knocked out, and is won immediately when every attacker is eliminated or when Round 3 completes with the Gunner alive. The optional **Hold Fast** bonus requires finishing with the Gunner at or above 50% HP.

**Mission 3 — Intercept Courier** (500 base + 150 bonus credits, unlocks Mission 4). Kill the Flanker-piloted Courier before it extracts at the marked exit cell or Round 5 completes with the Courier alive. The optional **Swift Intercept** bonus requires victory by the end of Round 2.

**Mission 4 — Breach the Gate** (600 base + 150 bonus credits, unlocks Mission 5). Destroy the Gate Bulwark; its escorts may be ignored, and the battle is won the moment the Bulwark falls. The optional **Chain Reaction** bonus requires damaging an enemy with enemy fire, collision, hazard, or explosion — the authored board offers an explosive the Gunner can detonate for splash and a hazard trench the Vanguard can ram the Bulwark into.

**Mission 5 — Crossfire Break** (700 base + 200 bonus credits, unlocks the Mission 6 handoff). Break the assault and destroy all enemies. The opening commits two Siege Mortar batteries whose Cross1 footprints share one cell, so pushing the displaced Controller into that shared cell walks it into both batteries' locked crossfire. The optional **Rapid Break** bonus rewards winning by the end of Round 4 but is not a failure deadline.

Missions 2–5 field the **Flanker** (HP 8, Move 4, Skirmish Carbine) alongside Riflemen, Strikers, and Artillery. The regular roster totals six archetypes: Rifleman, Striker, Artillery, Flanker, **Bulwark**, and **Controller**. The Bulwark (HP 16, Armor 4, Move 1, Bastion Cannon) is pushable — it has no displacement immunity. The Controller (HP 9, Armor 1, Move 2, initiative 35) carries the Impulse Projector (range 2–4, damage 3, Push 1): push-only behavior with no status system, and a displaced committed push whose live lane is lost resolves as the locked damage roll only. The Flanker acts on its own planner initiative between Strikers and Riflemen.

Objectives are labeled `PRIMARY` / `BONUS`; `[P]` is reserved for the pilot command.

For each Vanguard, Gunner, and Interceptor activation:

1. Click a player mech.
2. Optionally click **Move**, then a cyan destination cell.
3. Optionally click one of the mech's three weapons, hover or inspect the preview, then click an amber target cell.
4. Choose **Counter**, **Guard**, or **Evade**.
5. Click **Finish**.
6. After every surviving mech has finished, click **Resolve**.

Enemy footprints, expected damage, and hit chance remain locked throughout the player phase. Moving never retargets a committed attack. On defeat, use the visible **Restart Mission** button or press `R`; on victory, **CONTINUE** advances the campaign.

### Pilot skills

Each pilot has one signature skill usable once per mission, armed with the `[P] PILOT` command (`P` key) while that mech is the active unit:

- **Aegis** (Vanguard): shield a living orthogonally adjacent ally. For the entire next enemy resolution that ally takes the same 3-point post-armor reduction as Guard; Guard + Aegis does not stack.
- **Focus** (Gunner): the next validated player Action attack previews and resolves at 100% hit chance and is consumed on commit. Counter never consumes or receives Focus.
- **Overdrive** (Interceptor): before Move is spent, gain +2 movement for that activation. This is the only movement-range increase in the game — the Mobility upgrade track improves evasion instead.

### Keyboard mirrors

Pointer input selects mechs, destinations, and targets. These keys mirror the command buttons:

| Key | Command |
| --- | --- |
| `M` | Move |
| `1`, `2`, `3` | Weapon slots |
| `P` | Pilot skill (Aegis / Focus / Overdrive) |
| `C` | Counter |
| `G` | Guard |
| `E` | Evade |
| `F` | Finish unit |
| `Space` | Resolve committed attacks |
| `R` | Restart from a defeat (rejected on victory; victory shows **CONTINUE**) |

## Verify

The local commands match `.github/workflows/ci.yml`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Detailed evidence is recorded in `docs/validation/hpa-632.md`, `docs/validation/hpa-635.md`, `docs/validation/hpa-637.md`, and `docs/validation/hpa-523.md`.
