# Scorpius

Scorpius currently starts directly in **Mission 1: Turnabout at Relay Nine**, a retained desktop combat vertical slice built with Rust 2024 and Bevy 0.19.

## Run

Install a stable Rust toolchain that supports edition 2024, then run:

```bash
cargo run
```

There is no title screen, campaign flow, or save data yet. Those systems are intentionally outside HPA-632.

## Mission flow

The primary objective is to eliminate all four enemies. The optional **Turnabout** objective is achieved when an enemy takes damage from a committed enemy attack, collision, hazard, or explosion. The mission fails when all three player mechs are knocked out.

For each Vanguard, Gunner, and Interceptor activation:

1. Click a player mech.
2. Optionally click **Move**, then a cyan destination cell.
3. Optionally click one of the mech's three weapons, hover or inspect the preview, then click an amber target cell.
4. Choose **Counter**, **Guard**, or **Evade**.
5. Click **Finish**.
6. After every surviving mech has finished, click **Resolve**.

Enemy footprints, expected damage, and hit chance remain locked throughout the player phase. Moving never retargets a committed attack. On victory or defeat, use the visible **Restart Mission** button or press `R`.

### Keyboard mirrors

Pointer input selects mechs, destinations, and targets. These keys mirror the command buttons:

| Key | Command |
| --- | --- |
| `M` | Move |
| `1`, `2`, `3` | Weapon slots |
| `C` | Counter |
| `G` | Guard |
| `E` | Evade |
| `F` | Finish unit |
| `Space` | Resolve committed attacks |
| `R` | Restart from victory or defeat |

## Verify

The local commands match `.github/workflows/ci.yml`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Detailed HPA-632 evidence is recorded in `docs/validation/hpa-632.md`.
