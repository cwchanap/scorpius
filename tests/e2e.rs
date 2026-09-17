//! One critical rendered E2E flow: boot the real `scorpius` binary, go
//! Title -> New Game -> skip the pre-mission VN -> Start Mission, then select
//! Vanguard, arm MOVE, and click board cell (4, 8).
//!
//! The move is proven through BRP inspection of the Vanguard token's built-in
//! `bevy_ui::ui_node::Node`: its style `left`/`top` are rewritten from the
//! unit's grid position by the production sync/playback systems, and the
//! iso projection gives an exact, window-independent delta from (4, 7) to
//! (4, 8) of `-TILE_WIDTH / 2, +TILE_HEIGHT / 2`.
//!
//! Gated behind the `e2e` feature so `cargo test --all-targets` (which does
//! not enable it) compiles this file empty. Run rendered with:
//!
//! ```bash
//! cargo test --features e2e --test e2e -- --test-threads=1
//! ```
#![cfg(feature = "e2e")]

use std::{
    fs,
    path::PathBuf,
    process,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bevy_e2e::{E2eLaunchOptions, Error, Game, Result, cargo_bin, run};

/// Iso-projected screen delta from Vanguard's authored start cell (4, 7) to
/// the destination (4, 8): one iso step is half a tile left and down.
const MOVE_DELTA: (f64, f64) = (-56.0, 28.0);

#[test]
fn critical_flow_boots_to_mission_one_and_moves_vanguard() -> Result<()> {
    let (env_key, save_root) = isolated_save_root();
    let options = E2eLaunchOptions::new(cargo_bin!("scorpius"))
        .env(env_key, save_root.to_string_lossy().into_owned())
        // A cold debug Bevy binary can boot slowly under CI renderers.
        .startup_timeout(Duration::from_secs(60))
        .operation_timeout(Duration::from_secs(10));

    // Removes the temp save root on every exit path (normal return, `Err`
    // return, panic unwind). It removes only the save root; harness failure
    // artifacts live separately under `test_output/`, never under it.
    let _remove_save_root = RemoveOnDrop(save_root);

    run(options, |game| {
        // Title -> pre-mission VN -> briefing. Each hop re-clicks until the
        // next screen's marker appears, so a click lost to the cold-boot
        // layout race self-heals instead of timing out.
        click_through(game, "campaign.new_game", "campaign.skip_dialogue")?;
        click_through(game, "campaign.skip_dialogue", "campaign.start_mission")?;
        click_through(game, "campaign.start_mission", "battle.hud")?;

        // Battle: the Vanguard token must exist before interacting.
        game.wait_for("battle.unit.vanguard")?;
        let starting = vanguard_node_xy(game)?;

        // The menu rows exist from the first battle frame but only respond
        // once the Player phase begins, so poll: each pass is a no-op during
        // enemy planning, and the predicate only returns once the token has
        // reached (4, 8) — intermediate UnitMoved playback frames never
        // satisfy the destination delta.
        game.wait_until(Duration::from_secs(60), |game| {
            let xy = vanguard_node_xy(game)?;
            let (dx, dy) = (xy.0 - starting.0, xy.1 - starting.1);
            if (dx - MOVE_DELTA.0).abs() < 0.5 && (dy - MOVE_DELTA.1).abs() < 0.5 {
                return Ok(true);
            }
            game.click("battle.unit.vanguard")?;
            game.click("battle.action.move")?;
            game.click("battle.cell.4.8")?;
            Ok(false)
        })?;
        let landed = vanguard_node_xy(game)?;

        let (dx, dy) = (landed.0 - starting.0, landed.1 - starting.1);
        assert!(
            (dx - MOVE_DELTA.0).abs() < 0.5 && (dy - MOVE_DELTA.1).abs() < 0.5,
            "Vanguard token moved to the wrong rendered position: \
             start {starting:?}, landed {landed:?}, delta ({dx}, {dy}), \
             expected {MOVE_DELTA:?} for (4, 7) -> (4, 8)"
        );
        Ok(())
    })
}

/// Click `target` until the `expect` marker appears.
///
/// `wait_for` only proves the entity exists — on a cold boot the first poll
/// can land in the spawn frame, before `ui_layout` has computed the button's
/// `UiGlobalTransform`, so a fire-once click computes its coordinates from
/// the default transform and misses. Re-clicking until the next screen's
/// marker appears makes the navigation self-healing, mirroring the battle
/// loop's click-retry below.
fn click_through(game: &Game, target: &str, expect: &str) -> Result<()> {
    game.wait_for(target)?;
    game.wait_until(Duration::from_secs(10), |game| {
        if game.exists(expect)? {
            return Ok(true);
        }
        // The target can despawn mid-poll once navigation lands; the expect
        // marker then appears on a later pass.
        if game.exists(target)? {
            game.click(target)?;
        }
        Ok(false)
    })
}

/// Best-effort removal of the temporary save root on every exit path
/// (normal return, `Err` return, panic unwind). Removes only the save
/// root; harness failure artifacts live under `test_output/`, never here.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Unique temporary platform data root so the child never touches the real
/// campaign save; keyed by the same variable `SaveFile::platform_default`
/// reads on each platform.
fn isolated_save_root() -> (&'static str, PathBuf) {
    let unique = format!(
        "scorpius-e2e-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let key = if cfg!(target_os = "macos") {
        "HOME"
    } else if cfg!(windows) {
        "APPDATA"
    } else {
        "XDG_DATA_HOME"
    };
    (key, std::env::temp_dir().join(unique))
}

/// Read the Vanguard token's style position from its reflected `Node`.
fn vanguard_node_xy(game: &Game) -> Result<(f64, f64)> {
    let node = game.component_json("battle.unit.vanguard", "bevy_ui::ui_node::Node")?;
    let px = |key: &str| -> Result<f64> {
        node.get(key)
            .and_then(|value| value.get("Px"))
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| {
                Error::Configuration(format!(
                    "battle.unit.vanguard Node.{key} is not Val::Px: {node}"
                ))
            })
    };
    Ok((px("left")?, px("top")?))
}
