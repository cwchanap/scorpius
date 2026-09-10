//! Opt-in native UI capture for the HPA-480 visual parity matrix.
//!
//! The fixture table is deliberately Rust data.  This example only drives the
//! same typed campaign and battle seams that the production UI uses, then
//! requests Bevy's native image-target screenshot once the expected state is
//! visible.

mod ui_capture {
    pub mod fixtures;
}

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::Duration,
};

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    time::{Time, TimeUpdateStrategy, Virtual},
    window::PrimaryWindow,
};
use scorpius::{
    app::{GameScreen, ScorpiusPlugin},
    campaign::{
        model::{CampaignState, SquadUpgrades},
        save::SaveFile,
        session::{CampaignSession, complete_current_mission},
    },
    domain::{
        battle::BattleState,
        model::{BattleEvent, Faction, OptionalObjective, Reaction, UnitId},
    },
    mission::{MissionDefinition, mission_definition},
    presentation::{
        ActiveMission, AttackPreviewCells, BattleCamera2d, BattleEventQueue, BattleRuntime,
        CampaignCamera, CampaignRuntime, CanvasRoot, EventPlayback, RecentBattleLog,
        assets::{AssetLoadStatus, monitor_mission_assets},
        battle_menu::apply_menu_action,
        campaign_ui::{CampaignStatus, DialogueCursor, ScreenRoot, apply_campaign_action},
        interaction::{
            CommandAction, InteractionState, StatusMessage, execute_command, route_cell_click,
            route_token_click, update_hover_preview,
        },
        ui::{
            AssetStatusText, HudRoot, HudSnapshot, ObjectiveTrackSnapshot, format_event,
            update_asset_status_text,
        },
    },
};

use ui_capture::fixtures::{
    BattleSetup, CaptureAction, CaptureFixture, ExpectedFact, ObjectiveFact, SavePreset,
    UpgradePreset, fixture,
};

const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;

#[derive(Debug)]
struct CaptureArgs {
    scenario: String,
    width: u32,
    height: u32,
    seed: u64,
    time_ms: u64,
    output: PathBuf,
}

impl CaptureArgs {
    fn parse() -> Self {
        let mut scenario = None;
        let mut size = None;
        let mut seed = 0_u64;
        let mut time_ms = 0_u64;
        let mut output = None;
        let mut args = env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--scenario" => scenario = Some(next_arg(&mut args, "--scenario")),
                "--size" => size = Some(next_arg(&mut args, "--size")),
                "--seed" => {
                    seed = next_arg(&mut args, "--seed")
                        .parse()
                        .expect("--seed must be an unsigned integer");
                }
                "--time-ms" => {
                    time_ms = next_arg(&mut args, "--time-ms")
                        .parse()
                        .expect("--time-ms must be an unsigned integer");
                }
                "--output" => output = Some(PathBuf::from(next_arg(&mut args, "--output"))),
                "--help" | "-h" => {
                    println!(
                        "usage: ui_capture --scenario ID [--size WIDTHxHEIGHT] [--seed N] [--time-ms N] --output PATH"
                    );
                    process::exit(0);
                }
                unknown => panic!("unknown capture argument {unknown}"),
            }
        }

        let size = size.map(|size| parse_size(&size));
        let scenario = scenario.unwrap_or_else(|| panic!("--scenario is required"));
        let output = output.unwrap_or_else(|| panic!("--output is required"));
        Self {
            scenario,
            width: size.map_or(DEFAULT_WIDTH, |size| size.0),
            height: size.map_or(DEFAULT_HEIGHT, |size| size.1),
            seed,
            time_ms,
            output,
        }
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> String {
    args.next()
        .unwrap_or_else(|| panic!("{name} requires a value"))
}

fn parse_size(value: &str) -> (u32, u32) {
    let (width, height) = value
        .split_once('x')
        .unwrap_or_else(|| panic!("--size must use WIDTHxHEIGHT"));
    let width = width
        .parse()
        .expect("capture width must be an unsigned integer");
    let height = height
        .parse()
        .expect("capture height must be an unsigned integer");
    assert!(
        width > 0 && height > 0,
        "capture dimensions must be positive"
    );
    (width, height)
}

#[derive(Resource)]
struct CaptureRun {
    fixture: &'static CaptureFixture,
    output: PathBuf,
    seed: u64,
    width: u32,
    height: u32,
    time_ms: u64,
    deterministic_battle_installed: bool,
    actions_applied: bool,
    playback_event: Option<BattleEvent>,
    target_configured: bool,
    target_image: Option<Handle<Image>>,
    time_advanced: bool,
    validated: bool,
    render_warmup_frames: u8,
    capture_requested: bool,
    capture_finished: bool,
    capture_failed: bool,
}

fn main() {
    let args = CaptureArgs::parse();
    let fixture = fixture(&args.scenario)
        .unwrap_or_else(|| panic!("unknown capture scenario {:?}", args.scenario));
    let output_parent = args.output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_parent)
        .unwrap_or_else(|error| panic!("create capture output directory: {error}"));

    let save_path = env::temp_dir().join(format!(
        "scorpius-ui-capture-{}-{}.json",
        process::id(),
        fixture.id
    ));
    prepare_save(&save_path, fixture);
    let profile = fixture.profile;
    let state = CampaignState {
        next_mission: profile.next_mission,
        credits: profile.credits,
        upgrades: squad_upgrades(profile.upgrades),
        completed: profile.completed,
    };
    let save = SaveFile::new(save_path.clone());
    let session = CampaignSession {
        state: Some(state),
        save,
        last_completion: profile.receipt,
    };
    let definition = mission_definition(profile.mission)
        .unwrap_or_else(|| panic!("fixture mission {:?} has no definition", profile.mission));

    let mut app = App::new();
    app.add_plugins(ScorpiusPlugin)
        .insert_resource(CampaignRuntime(session))
        .insert_resource(ActiveMission(definition))
        .insert_resource(CaptureRun {
            fixture,
            output: args.output,
            seed: args.seed,
            width: args.width,
            height: args.height,
            time_ms: args.time_ms,
            deterministic_battle_installed: false,
            actions_applied: false,
            playback_event: None,
            target_configured: false,
            target_image: None,
            time_advanced: false,
            validated: false,
            render_warmup_frames: 0,
            capture_requested: false,
            capture_finished: false,
            capture_failed: false,
        })
        // Keep presentation time deterministic while allowing the native
        // window and asset/render pipelines to warm up over real frames.
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .add_systems(
            Startup,
            (initialize_capture, configure_capture_window).chain(),
        )
        .add_systems(PreUpdate, advance_capture_time)
        .add_systems(
            Update,
            (initialize_capture, drive_capture, exit_after_capture).chain(),
        );
    app.add_systems(
        Update,
        restore_capture_asset_status
            .after(monitor_mission_assets)
            .after(drive_capture)
            .before(update_asset_status_text),
    );
    app.add_systems(PostUpdate, finish_capture);
    let exit = app.run();

    let cleanup = if save_path.is_dir() {
        fs::remove_dir(&save_path)
    } else {
        fs::remove_file(&save_path)
    };
    if let Err(error) = cleanup
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("warning: could not remove isolated capture save: {error}");
    }
    if exit.is_error() {
        process::exit(1);
    }
}

fn prepare_save(path: &Path, fixture: &CaptureFixture) {
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(path).expect("remove stale capture save directory");
        } else {
            fs::remove_file(path).expect("remove stale capture save");
        }
    }
    if fixture.profile.save == SavePreset::Error {
        fs::create_dir_all(path).expect("create save-error fixture directory");
        return;
    }
    // The no-save title fixture intentionally exercises the disabled Continue
    // button. Every other fixture receives a real isolated save when the UI
    // needs to read one, while campaign screens still use the in-memory state.
    if fixture.id != "title-no-save" {
        let profile = fixture.profile;
        let state = CampaignState {
            next_mission: profile.next_mission,
            credits: profile.credits,
            upgrades: squad_upgrades(profile.upgrades),
            completed: profile.completed,
        };
        SaveFile::new(path.to_owned())
            .store(&state)
            .expect("store isolated capture save");
    }
}

fn squad_upgrades(preset: UpgradePreset) -> SquadUpgrades {
    SquadUpgrades {
        vanguard: preset.vanguard,
        gunner: preset.gunner,
        interceptor: preset.interceptor,
    }
}

fn initialize_capture(
    mut next_state: ResMut<NextState<GameScreen>>,
    state: Res<State<GameScreen>>,
    canvas_roots: Query<&ComputedNode, With<CanvasRoot>>,
    run: Res<CaptureRun>,
) {
    if run.fixture.profile.initial_screen != GameScreen::Title
        && *state.get() == GameScreen::Title
        && canvas_roots
            .iter()
            .any(|node| node.size.x > 0.0 && node.size.y > 0.0)
    {
        next_state.set(run.fixture.profile.initial_screen);
    }
}

fn configure_capture_window(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    run: Res<CaptureRun>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.resolution.set_scale_factor_override(Some(1.0));
    window
        .resolution
        .set_physical_resolution(run.width, run.height);
}

fn configure_window(world: &mut World, width: u32, height: u32) {
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut window) = windows.single_mut(world) {
        // A physical-pixel override makes the saved PNG dimensions stable on
        // the current HiDPI desktop and in CI.
        window.resolution.set_scale_factor_override(Some(1.0));
        window.resolution.set_physical_resolution(width, height);
    }
}

fn drive_capture(world: &mut World) {
    let screen = *world.resource::<State<GameScreen>>().get();
    let (fixture, needs_battle_setup) = {
        let run = world.resource::<CaptureRun>();
        (
            run.fixture,
            screen == GameScreen::Battle && !run.deterministic_battle_installed,
        )
    };
    if screen != fixture.profile.initial_screen {
        return;
    }
    if needs_battle_setup && world.get_resource::<BattleRuntime>().is_some() {
        install_authored_battle(world);
        world
            .resource_mut::<CaptureRun>()
            .deterministic_battle_installed = true;
    }

    if !screen_is_built(world, screen) {
        return;
    }
    if !fixture_has_asset_override(fixture)
        && !matches!(world.resource::<AssetLoadStatus>(), AssetLoadStatus::Ready)
    {
        return;
    }

    let actions_applied = world.resource::<CaptureRun>().actions_applied;
    if !actions_applied {
        let actions = fixture.actions;
        let seed = world.resource::<CaptureRun>().seed;
        for action in actions {
            apply_action(world, action, seed);
        }
        if !fixture_has_playback_setup(fixture) {
            // Direct setup actions have already committed their domain state;
            // start the screenshot from the production UI's settled boundary.
            world.resource_mut::<BattleEventQueue>().0.clear();
            *world.resource_mut::<EventPlayback>() = EventPlayback::default();
        }
        world.resource_mut::<CaptureRun>().actions_applied = true;
        return;
    }

    if let Some(event) = world.resource_mut::<CaptureRun>().playback_event.take() {
        *world.resource_mut::<EventPlayback>() = EventPlayback {
            current: Some((event, Timer::from_seconds(0.5, TimerMode::Once))),
            input_locked: true,
        };
    }

    if !world.resource::<CaptureRun>().target_configured && configure_capture_target(world) {
        world.resource_mut::<CaptureRun>().target_configured = true;
    }
}

fn restore_capture_asset_status(run: Res<CaptureRun>, mut status: ResMut<AssetLoadStatus>) {
    if !run.actions_applied {
        return;
    }
    if let Some(status_override) = run.fixture.actions.iter().find_map(|action| {
        let CaptureAction::SetAssetStatus(status) = action else {
            return None;
        };
        Some(status.clone())
    }) {
        *status = status_override;
    }
}

fn configure_capture_target(world: &mut World) -> bool {
    let (width, height, target_image) = {
        let run = world.resource::<CaptureRun>();
        (run.width, run.height, run.target_image.clone())
    };
    let target_image = target_image.unwrap_or_else(|| {
        let size = Extent3d {
            width,
            height,
            ..default()
        };
        let mut image = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let handle = world.resource_mut::<Assets<Image>>().add(image);
        world.resource_mut::<CaptureRun>().target_image = Some(handle.clone());
        handle
    });

    let screen = *world.resource::<State<GameScreen>>().get();
    let camera = match screen {
        GameScreen::Battle => world
            .query_filtered::<(Entity, &Camera), With<BattleCamera2d>>()
            .iter(world)
            .find(|(_, camera)| camera.is_active)
            .map(|(entity, _)| entity),
        _ => world
            .query_filtered::<(Entity, &Camera), With<CampaignCamera>>()
            .iter(world)
            .find(|(_, camera)| camera.is_active)
            .map(|(entity, _)| entity),
    };
    let Some(camera) = camera else {
        return false;
    };
    let target_canvas = match screen {
        GameScreen::Battle => world
            .query_filtered::<Entity, With<HudRoot>>()
            .iter(world)
            .next()
            .and_then(|root| world.get::<ChildOf>(root).map(ChildOf::parent)),
        _ => world
            .query_filtered::<Entity, With<ScreenRoot>>()
            .iter(world)
            .next()
            .and_then(|root| world.get::<ChildOf>(root).map(ChildOf::parent)),
    };
    let Some(target_canvas) = target_canvas else {
        return false;
    };
    let target_viewport = world
        .get::<ChildOf>(target_canvas)
        .map(ChildOf::parent)
        .expect("capture canvas must have a viewport parent");

    world
        .entity_mut(camera)
        .insert(RenderTarget::Image(target_image.into()));
    // Bevy's UI target follows the existing viewport/canvas hierarchy. The
    // target is configured on the active production camera; no capture-only
    // UI tree or camera is introduced.
    world
        .entity_mut(target_viewport)
        .insert((UiTargetCamera(camera), Visibility::Visible));
    true
}

fn advance_capture_time(
    mut run: ResMut<CaptureRun>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut time: ResMut<Time>,
) {
    if !run.actions_applied || run.time_advanced {
        return;
    }

    let duration = Duration::from_millis(run.time_ms);
    virtual_time.advance_by(duration);
    *time = virtual_time.as_generic();
    run.time_advanced = true;
}

fn finish_capture(world: &mut World) {
    let screen = *world.resource::<State<GameScreen>>().get();
    let fixture = world.resource::<CaptureRun>().fixture;
    if !screen_is_built(world, screen) {
        return;
    }
    if !fixture_has_asset_override(fixture)
        && !matches!(world.resource::<AssetLoadStatus>(), AssetLoadStatus::Ready)
    {
        return;
    }

    let (target_configured, time_advanced, validated, capture_requested, warmup_frames) = {
        let run = world.resource::<CaptureRun>();
        (
            run.target_configured,
            run.time_advanced,
            run.validated,
            run.capture_requested,
            run.render_warmup_frames,
        )
    };
    if !target_configured || !time_advanced || capture_requested {
        return;
    }
    if !validated {
        validate_fixture(world, fixture);
        world.resource_mut::<CaptureRun>().validated = true;
        return;
    }
    if warmup_frames < 2 {
        world.resource_mut::<CaptureRun>().render_warmup_frames += 1;
        return;
    }

    let (output, width, height, time_ms, target_image) = {
        let run = world.resource::<CaptureRun>();
        (
            run.output.clone(),
            run.width,
            run.height,
            run.time_ms,
            run.target_image
                .clone()
                .expect("capture target is configured"),
        )
    };
    eprintln!(
        "capture scenario={} screen={screen:?} physical={width}x{height} presentation_time_ms={time_ms} output={}",
        fixture.id,
        output.display()
    );
    let screenshot = world.spawn(Screenshot::image(target_image)).id();
    world.entity_mut(screenshot).observe(save_to_disk(output));
    world.entity_mut(screenshot).observe(capture_complete);
    world.resource_mut::<CaptureRun>().capture_requested = true;
}

fn capture_complete(event: On<ScreenshotCaptured>, mut run: ResMut<CaptureRun>) {
    match event.image.clone().try_into_dynamic() {
        Ok(image) => {
            let image = image.to_rgb8();
            if image.pixels().all(|pixel| pixel.0 == [0, 0, 0]) {
                eprintln!(
                    "capture failed: screenshot output={} is uniformly black; the native window must be visible and not fully occluded",
                    run.output.display()
                );
                run.capture_failed = true;
            } else {
                eprintln!("capture screenshot ready: output={}", run.output.display());
            }
        }
        Err(error) => {
            eprintln!(
                "capture failed: screenshot output={} could not be decoded: {error:?}",
                run.output.display()
            );
            run.capture_failed = true;
        }
    }
    run.capture_finished = true;
}

fn exit_after_capture(run: Res<CaptureRun>, mut exit: MessageWriter<AppExit>) {
    if run.capture_finished {
        exit.write(if run.capture_failed {
            AppExit::error()
        } else {
            AppExit::Success
        });
    }
}

fn screen_is_built(world: &mut World, screen: GameScreen) -> bool {
    match screen {
        GameScreen::Battle => world
            .query_filtered::<Entity, With<HudRoot>>()
            .iter(world)
            .next()
            .is_some(),
        GameScreen::Title
        | GameScreen::PreMissionStory
        | GameScreen::Briefing
        | GameScreen::Aftermath
        | GameScreen::Upgrade
        | GameScreen::Ending => world
            .query_filtered::<Entity, With<ScreenRoot>>()
            .iter(world)
            .next()
            .is_some(),
    }
}

fn fixture_has_asset_override(fixture: &CaptureFixture) -> bool {
    fixture
        .actions
        .iter()
        .any(|action| matches!(action, CaptureAction::SetAssetStatus(_)))
}

fn fixture_has_playback_setup(fixture: &CaptureFixture) -> bool {
    fixture
        .actions
        .iter()
        .any(|action| matches!(action, CaptureAction::SetBattle(BattleSetup::Playback)))
}

fn install_authored_battle(world: &mut World) {
    let definition = world.resource::<ActiveMission>().0;
    let upgrades = world
        .resource::<CampaignRuntime>()
        .0
        .state
        .as_ref()
        .expect("capture battle requires campaign state")
        .upgrades
        .clone();
    let seed = world.resource::<CaptureRun>().seed;
    let mut battle = (definition.build)(seed, &upgrades);
    battle
        .begin_round()
        .expect("authored capture opening must be legal");
    world.resource_mut::<BattleRuntime>().0 = battle;
}

fn apply_action(world: &mut World, action: &CaptureAction, seed: u64) {
    match action {
        CaptureAction::Campaign(action) => {
            let screen = *world.resource::<State<GameScreen>>().get();
            let active = world.get_resource::<ActiveMission>().copied();
            world.resource_scope(|world, mut runtime: Mut<CampaignRuntime>| {
                world.resource_scope(|world, mut cursor: Mut<DialogueCursor>| {
                    world.resource_scope(|world, mut status: Mut<CampaignStatus>| {
                        let mut next_state = world.resource_mut::<NextState<GameScreen>>();
                        apply_campaign_action(
                            *action,
                            screen,
                            &mut runtime,
                            active.as_ref(),
                            &mut cursor,
                            &mut status,
                            &mut next_state,
                        );
                    });
                });
            });
        }
        CaptureAction::Inspect(unit) => {
            let result = world.resource_scope(|world, mut battle: Mut<BattleRuntime>| {
                let mut interaction = world.resource_mut::<InteractionState>();
                route_token_click(&mut battle.0, &mut interaction, *unit)
            });
            handle_battle_events(world, result);
        }
        CaptureAction::Command(action) => apply_command(world, *action),
        CaptureAction::Menu(action) => {
            let applied = world.resource_scope(|world, battle: Mut<BattleRuntime>| {
                let mut interaction = world.resource_mut::<InteractionState>();
                apply_menu_action(&battle.0, &mut interaction, *action)
            });
            assert!(applied, "fixture menu action was rejected: {action:?}");
        }
        CaptureAction::ClickCell(cell) => {
            let result = world.resource_scope(|world, mut battle: Mut<BattleRuntime>| {
                let mut interaction = world.resource_mut::<InteractionState>();
                route_cell_click(&mut battle.0, &mut interaction, *cell)
            });
            handle_battle_events(world, result);
        }
        CaptureAction::HoverCell(cell) => {
            let footprint = world.resource_scope(|world, battle: Mut<BattleRuntime>| {
                let mut interaction = world.resource_mut::<InteractionState>();
                update_hover_preview(&battle.0, &mut interaction, *cell);
                interaction
                    .preview
                    .as_ref()
                    .map(|preview| preview.footprint.clone())
                    .unwrap_or_default()
            });
            let mut preview_cells = world.resource_mut::<AttackPreviewCells>();
            preview_cells.0.clear();
            preview_cells.0.extend(footprint);
        }
        CaptureAction::SetWindow(width, height) => {
            configure_window(world, *width, *height);
            let mut run = world.resource_mut::<CaptureRun>();
            run.width = *width;
            run.height = *height;
        }
        CaptureAction::SetBattle(setup) => install_battle_setup(world, *setup, seed),
        CaptureAction::SetAssetStatus(status) => {
            *world.resource_mut::<AssetLoadStatus>() = status.clone();
        }
        CaptureAction::AdvanceRounds(rounds) => advance_rounds(world, *rounds),
    }
}

fn apply_command(world: &mut World, action: CommandAction) {
    if action == CommandAction::ContinueVictory {
        let result = world.resource::<BattleRuntime>().0.result();
        let active = *world.resource::<ActiveMission>();
        let result = result.filter(|result| result.victory);
        if let Some(result) = result {
            let complete = world.resource_scope(|_world, mut runtime: Mut<CampaignRuntime>| {
                complete_current_mission(&mut runtime.0, active.0, result)
            });
            if let Err(error) = complete {
                world.resource_mut::<StatusMessage>().0 = error.to_string();
            }
        }
        return;
    }
    let result = world.resource_scope(|world, mut battle: Mut<BattleRuntime>| {
        let mut interaction = world.resource_mut::<InteractionState>();
        execute_command(&mut battle.0, &mut interaction, action)
    });
    handle_battle_events(world, result);
}

fn handle_battle_events(
    world: &mut World,
    result: Result<Vec<BattleEvent>, scorpius::domain::model::BattleError>,
) {
    match result {
        Ok(events) => {
            world.resource_mut::<EventPlayback>().input_locked |= !events.is_empty();
            world.resource_mut::<BattleEventQueue>().0.extend(events);
            world.resource_mut::<StatusMessage>().0.clear();
        }
        Err(error) => panic!("capture action rejected: {error}"),
    }
}

fn install_battle_setup(world: &mut World, setup: BattleSetup, seed: u64) {
    let definition = world.resource::<ActiveMission>().0;
    let upgrades = world
        .resource::<CampaignRuntime>()
        .0
        .state
        .as_ref()
        .expect("capture battle requires campaign state")
        .upgrades
        .clone();
    if let BattleSetup::SetBossBelowHp(target, hp) = setup
        && matches!(
            target,
            scorpius::mission::mission_six::ids::DREADNOUGHT
                | scorpius::mission::mission_seven::ids::REGENT
        )
    {
        let battle = build_boss_threshold_fixture(definition, &upgrades, seed, target, hp);
        world.resource_mut::<BattleRuntime>().0 = battle;
        *world.resource_mut::<InteractionState>() = InteractionState::default();
        world.resource_mut::<BattleEventQueue>().0.clear();
        *world.resource_mut::<EventPlayback>() = EventPlayback::default();
        world.resource_mut::<AttackPreviewCells>().0.clear();
        return;
    }
    let authored = (definition.build)(seed, &upgrades);
    let board = authored.board().clone();
    let weapons = authored.weapons().cloned().collect::<Vec<_>>();
    let rules = *authored.rules();
    let mut units = authored.units().cloned().collect::<Vec<_>>();
    match setup {
        BattleSetup::Authored => {}
        BattleSetup::SetUnitHp(target, hp) => {
            let unit = units
                .iter_mut()
                .find(|unit| unit.id == target)
                .unwrap_or_else(|| panic!("fixture HP target {target:?} is missing"));
            unit.hp = hp;
        }
        BattleSetup::Terminal(result) => {
            let knockout_faction = if result.victory {
                Faction::Enemy
            } else {
                Faction::Player
            };
            for unit in &mut units {
                if unit.faction == knockout_faction {
                    unit.hp = 0;
                }
            }
            if result.victory
                && result.optional_complete
                && matches!(rules.optional, OptionalObjective::Turnabout)
            {
                let player = units
                    .iter()
                    .find(|unit| unit.faction == Faction::Player)
                    .cloned()
                    .expect("terminal victory needs a player");
                // Keep the authored Turnabout rule. Choose the nearest authored
                // opening that lines up with the player, then use public push
                // actions after opening movement to produce real collision
                // damage and the optional objective event.
                let target_id = rules
                    .opening_plan
                    .iter()
                    .filter(|opening| {
                        opening.destination.x == player.position.x
                            || opening.destination.y == player.position.y
                    })
                    .min_by_key(|opening| opening.destination.manhattan(player.position))
                    .map(|opening| opening.unit)
                    .or_else(|| {
                        units
                            .iter()
                            .find(|unit| unit.faction == Faction::Enemy)
                            .map(|unit| unit.id)
                    })
                    .expect("terminal victory needs an enemy opening");
                units
                    .iter_mut()
                    .find(|unit| unit.id == target_id)
                    .expect("Turnabout target must exist")
                    .hp = 6;
            }
        }
        BattleSetup::SetBossBelowHp(..) => unreachable!("boss threshold fixtures return early"),
        BattleSetup::AuthoredWithLog | BattleSetup::Playback => {}
    }
    let mut battle = BattleState::new(board, units, weapons, rules, seed);
    let opening_events = if matches!(setup, BattleSetup::AuthoredWithLog | BattleSetup::Playback) {
        Some(
            battle
                .begin_round()
                .expect("typed playback battle setup must begin legally"),
        )
    } else {
        battle
            .begin_round()
            .expect("typed capture battle setup must begin legally");
        None
    };
    if let BattleSetup::Terminal(result) = setup
        && result.victory
        && result.optional_complete
        && matches!(battle.rules().optional, OptionalObjective::Turnabout)
    {
        let player = battle
            .units()
            .find(|unit| unit.faction == Faction::Player)
            .map(|unit| unit.id)
            .expect("terminal victory needs a player");
        let target = battle
            .units()
            .find(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .map(|unit| unit.id)
            .expect("terminal victory needs a living enemy");
        let mut collision_seen = false;
        for _ in 0..12 {
            let events = battle
                .resolve_push(player, target)
                .expect("Turnabout terminal fixture push must resolve");
            collision_seen |= events
                .iter()
                .any(|event| matches!(event, BattleEvent::CollisionOccurred { .. }));
            if battle.result().is_some() {
                break;
            }
        }
        assert!(
            collision_seen && battle.objectives().optional_complete,
            "Turnabout terminal fixture must produce a collision bonus"
        );
    }
    if let BattleSetup::Terminal(result) = setup
        && battle.result().is_none()
    {
        panic!("terminal fixture did not reach a terminal result: {result:?}");
    }
    let playback_event = if matches!(setup, BattleSetup::Playback) {
        let playback_unit = battle
            .units()
            .find(|unit| unit.faction == Faction::Player)
            .map(|unit| unit.id)
            .expect("playback fixture needs an authored player");
        let current_position = battle
            .unit(playback_unit)
            .expect("playback fixture player must exist")
            .position;
        let destination = battle
            .reachable_cells(playback_unit)
            .expect("playback fixture player reachability must be available")
            .into_iter()
            .find(|position| *position != current_position)
            .expect("playback fixture needs a legal movement destination");
        let events = battle
            .begin_activation(playback_unit)
            .and_then(|_| battle.move_unit(playback_unit, destination))
            .expect("playback fixture must use a legal Vanguard move");
        Some(
            events
                .into_iter()
                .find(|event| matches!(event, BattleEvent::UnitMoved { .. }))
                .expect("legal playback move must emit UnitMoved"),
        )
    } else {
        None
    };
    world.resource_mut::<BattleRuntime>().0 = battle;
    *world.resource_mut::<InteractionState>() = InteractionState::default();
    world.resource_mut::<BattleEventQueue>().0.clear();
    *world.resource_mut::<EventPlayback>() = EventPlayback::default();
    world.resource_mut::<AttackPreviewCells>().0.clear();
    if let Some(opening_events) = opening_events {
        let battle = &world.resource::<BattleRuntime>().0;
        let mut log = RecentBattleLog::default();
        for event in opening_events.iter().take(4) {
            log.push(format_event(event, battle));
        }
        assert_eq!(log.0.len(), 4, "playback fixture needs four opening events");
        *world.resource_mut::<RecentBattleLog>() = log;
    }
    if let Some(event) = playback_event {
        world.resource_mut::<CaptureRun>().playback_event = Some(event);
    }
}

fn build_boss_threshold_fixture(
    definition: &MissionDefinition,
    upgrades: &SquadUpgrades,
    seed: u64,
    boss: UnitId,
    desired_hp: i16,
) -> BattleState {
    let mut battle = (definition.build)(seed, upgrades);
    battle
        .begin_round()
        .expect("boss threshold fixture opening must be legal");

    let mut rounds = 0_u8;
    while battle.unit(boss).is_some_and(|unit| unit.hp > desired_hp) {
        rounds = rounds.saturating_add(1);
        assert!(
            rounds <= 12,
            "boss threshold fixture did not reach the requested HP in 12 rounds"
        );
        let boss_position = battle
            .unit(boss)
            .expect("boss threshold fixture target must exist")
            .position;
        let gunner = scorpius::mission::squad::ids::GUNNER;
        battle
            .begin_activation(gunner)
            .expect("boss threshold fixture Gunner activation must be legal");
        if !battle.pilot_skills().focus_used {
            battle
                .use_focus()
                .expect("boss threshold fixture Gunner Focus must be legal");
        }
        let weapon = if battle.unit(gunner).is_some_and(|unit| unit.en >= 5) {
            scorpius::mission::squad::ids::OVERCHARGE_SHOT
        } else {
            scorpius::mission::squad::ids::RAIL_RIFLE
        };
        let (min_range, max_range) = {
            let weapon = battle
                .weapon(weapon)
                .expect("authored Gunner threshold weapon must exist");
            (weapon.min_range, weapon.max_range)
        };
        let current_position = battle.unit(gunner).expect("Gunner must exist").position;
        let destination = battle
            .reachable_cells(gunner)
            .expect("Gunner reachability must be available")
            .into_iter()
            .find(|position| {
                let distance = position.manhattan(boss_position);
                distance >= min_range && distance <= max_range
            });
        if let Some(destination) = destination {
            if destination != current_position {
                battle
                    .move_unit(gunner, destination)
                    .expect("boss threshold fixture Gunner move must be legal");
            }
        } else {
            let distance = current_position.manhattan(boss_position);
            assert!(
                distance >= min_range && distance <= max_range,
                "boss threshold fixture cannot reach boss with Gunner at {current_position:?}"
            );
        }
        battle
            .attack(gunner, weapon, boss_position)
            .expect("boss threshold fixture Gunner attack must be legal");
        battle
            .choose_reaction(gunner, Reaction::Guard)
            .expect("boss threshold fixture Gunner reaction must be legal");
        battle
            .finish_activation(gunner)
            .expect("boss threshold fixture Gunner finish must be legal");

        for player in [
            scorpius::mission::squad::ids::VANGUARD,
            scorpius::mission::squad::ids::INTERCEPTOR,
        ] {
            battle
                .begin_activation(player)
                .expect("boss threshold fixture player activation must be legal");
            battle
                .choose_reaction(player, Reaction::Guard)
                .expect("boss threshold fixture player reaction must be legal");
            battle
                .finish_activation(player)
                .expect("boss threshold fixture player finish must be legal");
        }
        battle
            .resolve_enemy_phase()
            .expect("boss threshold fixture enemy phase must be legal");
    }

    assert!(
        battle
            .unit(boss)
            .is_some_and(|unit| { unit.hp > 0 && unit.hp <= desired_hp }),
        "boss threshold fixture must retain a living boss at or below the requested HP"
    );
    assert_eq!(battle.phase(), scorpius::domain::model::BattlePhase::Player);
    battle
}

fn advance_rounds(world: &mut World, rounds: u8) {
    for _ in 0..rounds {
        prepare_round_resolution(world);
        let result = world
            .resource_mut::<BattleRuntime>()
            .0
            .resolve_enemy_phase();
        handle_battle_events(world, result);
    }
    if rounds == 0 {
        prepare_round_resolution(world);
    }
}

fn prepare_round_resolution(world: &mut World) {
    let player_ids = world
        .resource::<BattleRuntime>()
        .0
        .units()
        .filter(|unit| {
            unit.faction == Faction::Player && !unit.is_knocked_out() && !unit.activation.finished
        })
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    for id in player_ids {
        let result = {
            let battle = &mut world.resource_mut::<BattleRuntime>().0;
            battle.begin_activation(id).and_then(|()| {
                battle.choose_reaction(id, Reaction::Guard)?;
                battle.finish_activation(id)
            })
        };
        if let Err(error) = result {
            panic!("fixture round preparation failed for {id:?}: {error}");
        }
    }
}

fn validate_fixture(world: &mut World, fixture: &CaptureFixture) {
    let screen = *world.resource::<State<GameScreen>>().get();
    for expected in fixture.expect {
        validate_fact(world, screen, expected);
    }
}

fn validate_fact(world: &mut World, screen: GameScreen, expected: &ExpectedFact) {
    match expected {
        ExpectedFact::Screen(want) => assert_eq!(screen, *want),
        ExpectedFact::BattlePhase(want) => assert_eq!(battle(world).phase(), *want),
        ExpectedFact::Round(want) => assert_eq!(battle(world).round(), *want),
        ExpectedFact::Menu(want) => assert_eq!(world.resource::<InteractionState>().menu, *want),
        ExpectedFact::Mode(want) => assert_eq!(world.resource::<InteractionState>().mode, *want),
        ExpectedFact::Inspection(want) => {
            assert_eq!(world.resource::<InteractionState>().inspected_unit, *want)
        }
        ExpectedFact::Dialogue {
            cursor,
            speaker,
            text,
        } => {
            assert_eq!(world.resource::<DialogueCursor>().0, *cursor);
            let active = *world.resource::<ActiveMission>();
            let scene = match screen {
                GameScreen::PreMissionStory => active.0.pre_mission,
                GameScreen::Aftermath => active.0.aftermath,
                other => panic!("dialogue fact is invalid on {other:?}"),
            };
            let line = scene.lines.get(*cursor).unwrap_or_else(|| {
                panic!("dialogue cursor {cursor} is outside the authored scene")
            });
            assert_eq!((line.speaker, line.text), (*speaker, *text));
        }
        ExpectedFact::UnitHp(unit, want) => {
            assert_eq!(battle(world).unit(*unit).map(|unit| unit.hp), Some(*want))
        }
        ExpectedFact::Primary(want) => assert_eq!(battle(world).rules().primary, *want),
        ExpectedFact::ObjectiveTrack(want) => validate_objective_track(world, want),
        ExpectedFact::IntentWeapon(unit, want) => assert_eq!(
            battle(world)
                .intent_for(*unit)
                .map(|intent| intent.profile.weapon),
            Some(*want)
        ),
        ExpectedFact::PilotFocusPending(want) => {
            assert_eq!(battle(world).pilot_skills().focus_pending, *want)
        }
        ExpectedFact::PilotOverdriveActive(want) => {
            assert_eq!(battle(world).pilot_skills().overdrive_active, *want)
        }
        ExpectedFact::ReadyToResolve(want) => assert_eq!(battle(world).ready_to_resolve(), *want),
        ExpectedFact::Result(want) => assert_eq!(battle(world).result(), *want),
        ExpectedFact::Credits(want) => assert_eq!(
            world
                .resource::<CampaignRuntime>()
                .0
                .state
                .as_ref()
                .map(|state| state.credits),
            Some(*want)
        ),
        ExpectedFact::RecentLogEntries(want) => {
            assert_eq!(world.resource::<RecentBattleLog>().0.len(), *want)
        }
        ExpectedFact::PlaybackActive(want) => {
            assert_eq!(world.resource::<EventPlayback>().current.is_some(), *want)
        }
        ExpectedFact::AssetStatus(want) => {
            assert_eq!(world.resource::<AssetLoadStatus>(), want);
            validate_asset_status_node(world, want);
        }
    }
}

fn validate_asset_status_node(world: &mut World, expected: &AssetLoadStatus) {
    let (text, visibility) = world
        .query_filtered::<(&Text, &Visibility), With<AssetStatusText>>()
        .single(world)
        .expect("battle asset status node must exist");
    assert_eq!(*visibility, Visibility::Visible);
    match expected {
        AssetLoadStatus::Loading => assert_eq!(text.0, "Loading battle UI assets..."),
        AssetLoadStatus::Failed(path) => {
            assert_eq!(text.0, format!("ASSET LOAD FAILED\n{path}"));
        }
        AssetLoadStatus::Ready => unreachable!("ready status has no visible asset node"),
    }
}

fn battle(world: &World) -> &BattleState {
    &world.resource::<BattleRuntime>().0
}

fn validate_objective_track(world: &mut World, expected: &ObjectiveFact) {
    let active = *world.resource::<ActiveMission>();
    let selected = world.resource::<InteractionState>().inspected_unit;
    let snapshot = HudSnapshot::from_battle(battle(world), selected, active.0);
    let actual = snapshot
        .objective_track
        .as_ref()
        .unwrap_or_else(|| panic!("expected objective track for {expected:?}"));
    match (expected, actual) {
        (
            ObjectiveFact::EliminateAll { remaining, total },
            ObjectiveTrackSnapshot::EliminateAll {
                remaining: actual_remaining,
                total: actual_total,
            },
        ) => assert_eq!((*actual_remaining, *actual_total), (*remaining, *total)),
        (
            ObjectiveFact::Protect {
                target,
                hp,
                max_hp,
                round,
            },
            ObjectiveTrackSnapshot::Protect {
                hp: actual_hp,
                max_hp: actual_max_hp,
                round: actual_round,
                ..
            },
        ) => {
            assert_eq!(primary_target(world), Some(*target));
            assert_eq!(
                (*actual_hp, *actual_max_hp, *actual_round),
                (*hp, *max_hp, *round)
            );
        }
        (
            ObjectiveFact::Intercept {
                target,
                distance,
                deadline_round,
                escape,
            },
            ObjectiveTrackSnapshot::Intercept {
                distance: actual_distance,
                deadline_round: actual_deadline,
                escape: actual_escape,
                ..
            },
        ) => {
            assert_eq!(primary_target(world), Some(*target));
            assert_eq!(
                (*actual_distance, *actual_deadline, *actual_escape),
                (*distance, *deadline_round, *escape)
            );
        }
        (
            ObjectiveFact::Target { target, hp, max_hp },
            ObjectiveTrackSnapshot::Target {
                hp: actual_hp,
                max_hp: actual_max_hp,
                ..
            },
        ) => {
            assert_eq!(primary_target(world), Some(*target));
            assert_eq!((*actual_hp, *actual_max_hp), (*hp, *max_hp));
        }
        (want, actual) => panic!("objective track mismatch: expected {want:?}, actual {actual:?}"),
    }
}

fn primary_target(world: &World) -> Option<scorpius::domain::model::UnitId> {
    match battle(world).rules().primary {
        scorpius::domain::model::PrimaryObjective::EliminateTarget { target }
        | scorpius::domain::model::PrimaryObjective::ProtectThroughRound { target, .. }
        | scorpius::domain::model::PrimaryObjective::InterceptBeforeEscape { target, .. } => {
            Some(target)
        }
        scorpius::domain::model::PrimaryObjective::EliminateAllEnemies => None,
    }
}
