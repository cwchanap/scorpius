use bevy::prelude::*;

use crate::domain::{board::GridPos, model::Faction};

use super::{
    BattleCamera2d, BattleRuntime, BattleStage, CanvasRoot, CellInsetVisual, CellVisual,
    PresentationNeedsRebuild, PresentationRoot, PropVisual, TokenAwaiting, TokenCard,
    TokenFootprintVisual, TokenHpFill, TokenHpText, TokenSelectionVisual, UnitVisual,
    assets::UiAssets,
    interaction::{
        on_battlefield_stage_click, on_battlefield_stage_move, on_battlefield_stage_out,
        on_battlefield_token_click, on_battlefield_token_move, on_battlefield_token_out,
    },
    layout::{
        BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_STAGE_SIZE, BLOCK_HEIGHT, TILE_HEIGHT,
        TILE_WIDTH, TOKEN_HEIGHT, TOKEN_WIDTH, battle_stage_rect, depth_key, iso_center,
    },
    theme,
};

pub fn mission_grid_cells(width: u8, height: u8) -> Vec<GridPos> {
    assert_eq!(width, BATTLE_GRID_WIDTH, "HPA-480 battle boards are 9x9");
    assert_eq!(height, BATTLE_GRID_HEIGHT, "HPA-480 battle boards are 9x9");
    (0..height)
        .flat_map(|y| (0..width).map(move |x| GridPos::new(x, y)))
        .collect()
}

pub fn setup_mission_scene(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    assert_authored_board(&battle.0);
    let canvas = canvas_roots
        .iter()
        .next()
        .unwrap_or_else(|| super::layout::spawn_canvas_root(&mut commands));

    commands.spawn((Camera2d, BattleCamera2d, UiPickingCamera));

    let root = spawn_presentation_root(&mut commands, canvas);
    populate_mission_root(&mut commands, root, &ui_assets, &battle);
}

pub(crate) fn rebuild_mission_scene(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    ui_assets: Res<UiAssets>,
    roots: Query<Entity, (With<PresentationRoot>, With<PresentationNeedsRebuild>)>,
) {
    assert_authored_board(&battle.0);
    for root in &roots {
        populate_mission_root(&mut commands, root, &ui_assets, &battle);
        commands.entity(root).remove::<PresentationNeedsRebuild>();
    }
}

fn assert_authored_board(battle: &crate::domain::battle::BattleState) {
    assert_eq!(
        battle.board().width(),
        BATTLE_GRID_WIDTH,
        "HPA-480 battle boards are 9x9"
    );
    assert_eq!(
        battle.board().height(),
        BATTLE_GRID_HEIGHT,
        "HPA-480 battle boards are 9x9"
    );
}

fn spawn_presentation_root(commands: &mut Commands, canvas: Entity) -> Entity {
    commands
        .spawn((
            Name::new("Battle Presentation"),
            PresentationRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(canvas),
        ))
        .id()
}

fn populate_mission_root(
    commands: &mut Commands,
    root: Entity,
    ui_assets: &UiAssets,
    battle: &BattleRuntime,
) {
    let stage = commands
        .spawn((
            Name::new("Battle Stage"),
            BattleStage,
            Node {
                position_type: PositionType::Absolute,
                left: px(battle_stage_rect().min.x),
                top: px(battle_stage_rect().min.y),
                width: px(BATTLE_STAGE_SIZE.x),
                height: px(BATTLE_STAGE_SIZE.y),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(5, 8, 15)),
            Pickable::default(),
            ChildOf(root),
        ))
        .observe(on_battlefield_stage_click)
        .observe(on_battlefield_stage_move)
        .observe(on_battlefield_stage_out)
        .id();

    for cell in mission_grid_cells(BATTLE_GRID_WIDTH, BATTLE_GRID_HEIGHT) {
        let fill = if (cell.x + cell.y) % 2 == 0 {
            theme::BOARD_LIGHT
        } else {
            theme::BOARD_DARK
        };
        let outer = commands
            .spawn((
                Name::new(format!("Cell {},{}", cell.x, cell.y)),
                cell_node(cell),
                theme::board_node(
                    ui_assets.board.clone(),
                    theme::BOARD_DIAMOND_RECT,
                    theme::BOARD_STROKE,
                ),
                CellVisual(cell),
                ZIndex(0),
                ChildOf(stage),
            ))
            .id();
        commands.spawn((
            cell_inset_node(),
            theme::board_node(ui_assets.board.clone(), theme::BOARD_DIAMOND_RECT, fill),
            CellInsetVisual(cell),
            Pickable::IGNORE,
            ChildOf(outer),
        ));
    }

    for cell in battle.0.board().blocking_cells() {
        spawn_blocker(commands, stage, ui_assets, cell);
    }
    for explosive in battle.0.board().explosives() {
        spawn_prop(
            commands,
            stage,
            ui_assets,
            PropVisual::Explosive(explosive.position),
            theme::BOARD_EXPLOSIVE,
            theme::ICON_ATTACK,
            !explosive.exploded,
        );
    }
    for cell in battle.0.board().hazard_cells() {
        spawn_prop(
            commands,
            stage,
            ui_assets,
            PropVisual::Hazard(cell),
            theme::BOARD_HAZARD,
            theme::ICON_GUARD,
            true,
        );
    }
    for unit in battle.0.units() {
        spawn_token(commands, stage, ui_assets, unit);
    }
}

fn cell_node(cell: GridPos) -> Node {
    let center = stage_point(cell);
    Node {
        position_type: PositionType::Absolute,
        left: px(center.x - 56.0),
        top: px(center.y - 28.0),
        width: px(112.0),
        height: px(56.0),
        ..default()
    }
}

fn cell_inset_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(3.0),
        top: px(3.0),
        width: px(TILE_WIDTH - 6.0),
        height: px(TILE_HEIGHT - 6.0),
        ..default()
    }
}

fn stage_point(cell: GridPos) -> Vec2 {
    iso_center(cell) - battle_stage_rect().min
}

fn spawn_blocker(commands: &mut Commands, stage: Entity, ui_assets: &UiAssets, cell: GridPos) {
    let center = stage_point(cell);
    commands.spawn((
        Name::new(format!("Blocking {},{}", cell.x, cell.y)),
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - TILE_WIDTH * 0.5),
            top: px(center.y - TILE_HEIGHT * 0.5 - BLOCK_HEIGHT),
            width: px(TILE_WIDTH),
            height: px(TILE_HEIGHT + BLOCK_HEIGHT),
            ..default()
        },
        theme::board_node(
            ui_assets.board.clone(),
            theme::BOARD_BLOCKER_RECT,
            Color::srgb_u8(28, 42, 58),
        ),
        PropVisual::Blocking(cell),
        ZIndex(10 + i32::from(depth_key(cell)) * 3),
        Pickable::IGNORE,
        ChildOf(stage),
    ));
}

fn spawn_prop(
    commands: &mut Commands,
    stage: Entity,
    ui_assets: &UiAssets,
    prop: PropVisual,
    tint: Color,
    icon_rect: Rect,
    visible: bool,
) {
    let cell = match prop {
        PropVisual::Blocking(cell) | PropVisual::Explosive(cell) | PropVisual::Hazard(cell) => cell,
    };
    let center = stage_point(cell);
    let root = commands
        .spawn((
            Name::new(format!("Board prop {},{}", cell.x, cell.y)),
            Node {
                position_type: PositionType::Absolute,
                left: px(center.x - 56.0),
                top: px(center.y - 28.0),
                width: px(112.0),
                height: px(56.0),
                ..default()
            },
            theme::board_node(ui_assets.board.clone(), theme::BOARD_DIAMOND_RECT, tint),
            prop,
            Visibility::Visible,
            ZIndex(6),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    if !visible {
        commands.entity(root).insert(Visibility::Hidden);
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(41.0),
            top: px(13.0),
            width: px(30.0),
            height: px(30.0),
            ..default()
        },
        theme::icon_node(ui_assets.icons.clone(), icon_rect, Color::WHITE),
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

fn spawn_token(
    commands: &mut Commands,
    stage: Entity,
    ui_assets: &UiAssets,
    unit: &crate::domain::model::UnitState,
) {
    let center = stage_point(unit.position);
    let style = theme::unit_archetype_style(unit.archetype);
    let depth = 11 + i32::from(depth_key(unit.position)) * 3;
    commands.spawn((
        Name::new(format!("{} footprint", unit.name)),
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - 56.0),
            // The packed shadow ellipse is centered at local y=69 in its
            // 112x96 atlas rect; top=cy-68 places that ellipse at cy+1.
            top: px(center.y - 68.0),
            width: px(112.0),
            height: px(96.0),
            ..default()
        },
        theme::board_node(
            ui_assets.board.clone(),
            style.footprint_rect,
            Color::srgba(0.0, 0.0, 0.0, 0.58),
        ),
        TokenFootprintVisual(unit.id),
        Visibility::Visible,
        ZIndex(depth - 1),
        Pickable::IGNORE,
        ChildOf(stage),
    ));

    let selection = commands
        .spawn((
            Name::new(format!("{} selection footprint", unit.name)),
            token_selection_node(unit.position),
            theme::board_node(
                ui_assets.board.clone(),
                theme::BOARD_DIAMOND_RECT,
                theme::BOARD_SELECTED,
            ),
            TokenSelectionVisual(unit.id),
            Visibility::Hidden,
            ZIndex(depth),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(3.0),
            top: px(3.0),
            width: px(TILE_WIDTH - 6.0),
            height: px(TILE_HEIGHT - 6.0),
            ..default()
        },
        theme::board_node(
            ui_assets.board.clone(),
            theme::BOARD_DIAMOND_RECT,
            theme::BOARD_DARK,
        ),
        Pickable::IGNORE,
        ChildOf(selection),
    ));

    let card = commands
        .spawn((
            Name::new(unit.name),
            UnitVisual(unit.id),
            TokenCard(unit.id),
            token_card_node(unit.position),
            UiTransform::IDENTITY,
            BackgroundColor(if unit.faction == Faction::Player {
                Color::srgba(0.08, 0.20, 0.29, 0.98)
            } else {
                Color::srgba(0.22, 0.08, 0.06, 0.98)
            }),
            Visibility::Visible,
            ZIndex(depth),
            Pickable::default(),
            ChildOf(stage),
        ))
        .observe(on_battlefield_token_click)
        .observe(on_battlefield_token_move)
        .observe(on_battlefield_token_out)
        .id();

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(23.0),
            top: px(20.0),
            width: px(30.0),
            height: px(30.0),
            ..default()
        },
        theme::icon_node(ui_assets.icons.clone(), style.glyph_rect, style.color),
        Pickable::IGNORE,
        ChildOf(card),
    ));

    let hp_bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(7.0),
                top: px(8.0),
                width: px(TOKEN_WIDTH - 14.0),
                height: px(6.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(7, 16, 26)),
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0 * f32::from(unit.hp.max(0)) / f32::from(unit.stats.max_hp)),
            height: px(6.0),
            ..default()
        },
        BackgroundColor(if unit.faction == Faction::Player {
            theme::MINT
        } else {
            theme::ENEMY
        }),
        TokenHpFill(unit.id),
        Pickable::IGNORE,
        ChildOf(hp_bar),
    ));
    commands.spawn((
        Text::new(unit.hp.to_string()),
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(600)),
        TextColor(if unit.faction == Faction::Player {
            Color::srgb_u8(168, 224, 188)
        } else {
            Color::srgb_u8(255, 179, 170)
        }),
        Node {
            position_type: PositionType::Absolute,
            right: px(6.0),
            top: px(18.0),
            ..default()
        },
        TokenHpText(unit.id),
        Pickable::IGNORE,
        ChildOf(card),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(6.0),
            top: px(6.0),
            width: px(9.0),
            height: px(9.0),
            ..default()
        },
        BackgroundColor(theme::GOLD),
        TokenAwaiting(unit.id),
        Visibility::Visible,
        Pickable::IGNORE,
        ChildOf(card),
    ));
}

fn token_card_node(position: GridPos) -> Node {
    let center = stage_point(position);
    Node {
        position_type: PositionType::Absolute,
        left: px(center.x - TOKEN_WIDTH * 0.5),
        top: px(center.y - TOKEN_HEIGHT - 4.0),
        width: px(TOKEN_WIDTH),
        height: px(TOKEN_HEIGHT),
        ..default()
    }
}

fn token_selection_node(position: GridPos) -> Node {
    let center = stage_point(position);
    Node {
        position_type: PositionType::Absolute,
        left: px(center.x - TILE_WIDTH * 0.5),
        top: px(center.y - TILE_HEIGHT * 0.5),
        width: px(TILE_WIDTH),
        height: px(TILE_HEIGHT),
        ..default()
    }
}
