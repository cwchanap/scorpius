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

    let outcome = run(options, |game| {
        // Title -> pre-mission VN -> briefing.
        game.wait_for("campaign.new_game")?;
        game.click("campaign.new_game")?;
        game.wait_for("campaign.skip_dialogue")?;
        game.click("campaign.skip_dialogue")?;
        game.wait_for("campaign.start_mission")?;
        game.click("campaign.start_mission")?;

        // Battle: the HUD and the Vanguard token must exist before interacting.
        game.wait_for("battle.hud")?;
        game.wait_for("battle.unit.vanguard")?;
        let starting = vanguard_node_xy(game)?;

        // The menu rows exist from the first battle frame but only respond
        // once the Player phase begins, so poll: each pass is a no-op during
        // enemy planning, and a successful pass ends with the token settled
        // on (4, 8) after the UnitMoved playback finishes.
        let mut previous: Option<(f64, f64)> = None;
        game.wait_until(Duration::from_secs(60), |game| {
            let xy = vanguard_node_xy(game)?;
            let settled = previous == Some(xy) && xy != starting;
            previous = Some(xy);
            if settled {
                return Ok(true);
            }
            game.click("battle.unit.vanguard")?;
            game.click("battle.action.move")?;
            game.click("battle.cell.4.8")?;
            Ok(false)
        })?;
        game.wait_frames(5)?;
        let landed = vanguard_node_xy(game)?;

        let (dx, dy) = (landed.0 - starting.0, landed.1 - starting.1);
        assert!(
            (dx - MOVE_DELTA.0).abs() < 0.5 && (dy - MOVE_DELTA.1).abs() < 0.5,
            "Vanguard token moved to the wrong rendered position: \
             start {starting:?}, landed {landed:?}, delta ({dx}, {dy}), \
             expected {MOVE_DELTA:?} for (4, 7) -> (4, 8)"
        );
        Ok(())
    });

    // Best-effort cleanup; `run` has already shut the child down on every
    // return path, and a panic unwinds past this line by design so the
    // failure artifacts under `test_output/` stay available.
    let _ = fs::remove_dir_all(save_root);
    outcome
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
