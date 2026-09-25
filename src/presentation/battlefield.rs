use bevy::prelude::*;
use std::collections::BTreeSet;

use crate::domain::{board::GridPos, model::Faction};

use super::{
    BattleCamera2d, BattleMap, BattleRuntime, BattleStage, CanvasRoot, CellInsetVisual, CellVisual,
    PresentationNeedsRebuild, PresentationRoot, PropVisual, TokenAwaiting, TokenCard,
    TokenFootprintVisual, TokenHpFill, TokenHpText, TokenSelectionVisual, UnitVisual,
    assets::UiAssets,
    e2e_id,
    interaction::{
        on_battlefield_stage_click, on_battlefield_stage_move, on_battlefield_stage_out,
        on_battlefield_token_click, on_battlefield_token_move, on_battlefield_token_out,
    },
    layout::{
        BATTLE_STAGE_SIZE, BLOCK_HEIGHT, MAP_UNIT_HEIGHT, MAP_UNIT_WIDTH, TILE_HEIGHT, TILE_WIDTH,
        battle_stage_rect, depth_key, footprint_top_left, iso_center, selection_top_left,
        token_depth, unit_root_top_left,
    },
    map_view::{MapView, spawn_map_controls},
    theme,
};

pub fn mission_grid_cells(width: u8, height: u8) -> Vec<GridPos> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| GridPos::new(x, y)))
        .collect()
}

pub fn setup_mission_scene(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    let canvas = canvas_roots
        .iter()
        .next()
        .unwrap_or_else(|| super::layout::spawn_canvas_root(&mut commands));

    commands.spawn((Camera2d, BattleCamera2d, UiPickingCamera));

    let root = spawn_presentation_root(&mut commands, canvas);
    populate_mission_root(
        &mut commands,
        root,
        &ui_assets,
        &battle,
        images.as_deref_mut(),
    );
}

pub(crate) fn rebuild_mission_scene(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    ui_assets: Res<UiAssets>,
    roots: Query<Entity, (With<PresentationRoot>, With<PresentationNeedsRebuild>)>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    for root in &roots {
        populate_mission_root(
            &mut commands,
            root,
            &ui_assets,
            &battle,
            images.as_deref_mut(),
        );
        commands.entity(root).remove::<PresentationNeedsRebuild>();
    }
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
    images: Option<&mut Assets<Image>>,
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

    let mut view = MapView::new(battle.0.board());
    if view.is_regional() {
        view.zoom = 0.7;
        // Focus the squad, not a hard-wired cell: deployments differ per
        // mission and the camera should open on the action.
        let focus = battle
            .0
            .units()
            .find(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
            .map_or(GridPos::new(0, 0), |unit| unit.position);
        view.focus(focus);
        if let Some(images) = images {
            spawn_map_controls(commands, stage, ui_assets, battle.0.board(), battle, images);
        }
    }
    let stage = commands
        .spawn((
            Name::new("Battle Map Content"),
            BattleMap,
            Node {
                position_type: PositionType::Absolute,
                width: px(BATTLE_STAGE_SIZE.x),
                height: px(BATTLE_STAGE_SIZE.y),
                ..default()
            },
            view.transform(),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    for cell in view.visible_cells() {
        spawn_cell(commands, stage, ui_assets, battle, cell);
    }
    commands.insert_resource(view);

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

pub fn reconcile_visible_cells(
    mut commands: Commands,
    view: Res<MapView>,
    battle: Res<BattleRuntime>,
    assets: Res<UiAssets>,
    maps: Query<Entity, With<BattleMap>>,
    existing: Query<(Entity, &CellVisual)>,
    mut previous: Local<Option<MapView>>,
) {
    if *previous == Some(*view) && !existing.is_empty() {
        return;
    }
    let Some(parent) = maps.iter().next() else {
        return;
    };
    let visible: BTreeSet<_> = view.visible_cells().into_iter().collect();
    let mut present = BTreeSet::new();
    for (entity, cell) in &existing {
        if visible.contains(&cell.0) {
            present.insert(cell.0);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for cell in visible.difference(&present) {
        spawn_cell(&mut commands, parent, &assets, &battle, *cell);
    }
    *previous = Some(*view);
}

fn spawn_cell(
    commands: &mut Commands,
    stage: Entity,
    assets: &UiAssets,
    battle: &BattleRuntime,
    cell: GridPos,
) {
    let board = battle.0.board();
    let regional = super::map_view::is_regional(board.width(), board.height());
    let terrain = board.terrain_at(cell).unwrap();
    let fill = super::map_view::cell_base_fill(board, cell);
    let art = regional.then(|| theme::terrain_art(terrain)).flatten();
    // Flat cells stay below every stage marker; tall terrain (art rising past
    // the diamond's authored 53px baseline, e.g. Forest/Mountain at 70) must
    // occlude tokens behind it, so it sorts just under the token slice the
    // way blockers already do.
    let z = if art.is_some_and(|(_, height)| height > 53.0) {
        token_depth(cell) - 2
    } else if regional {
        let max_depth = i32::from(depth_key(GridPos::new(
            board.width() - 1,
            board.height() - 1,
        )));
        i32::from(depth_key(cell)) - (max_depth + 2)
    } else {
        0
    };
    let outer = commands
        .spawn((
            Name::new(format!("Cell {},{}", cell.x, cell.y)),
            cell_node(cell),
            theme::board_node(
                assets.board.clone(),
                theme::BOARD_DIAMOND_RECT,
                theme::BOARD_STROKE,
            ),
            CellVisual(cell),
            ZIndex(z),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    e2e_id(
        commands,
        outer,
        (cell == GridPos::new(4, 8)).then_some("battle.cell.4.8"),
    );
    let (node, image) = if let Some((rect, height)) = art {
        (
            Node {
                top: px(53.0 - height),
                height: px(height),
                ..cell_inset_node()
            },
            theme::board_node(assets.terrain.clone(), rect, Color::WHITE),
        )
    } else {
        (
            cell_inset_node(),
            theme::board_node(assets.board.clone(), theme::BOARD_DIAMOND_RECT, fill),
        )
    };
    commands.spawn((
        node,
        image,
        CellInsetVisual(cell),
        Pickable::IGNORE,
        ChildOf(outer),
    ));
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
    let style = theme::unit_archetype_style(unit.archetype);
    let depth = token_depth(unit.position);
    let footprint = footprint_top_left(unit.position);
    commands.spawn((
        Name::new(format!("{} footprint", unit.name)),
        Node {
            position_type: PositionType::Absolute,
            left: px(footprint.x),
            // The packed shadow ellipse is centered at local y=69 in its
            // 112x96 atlas rect; the helper's top=cy-68 places that ellipse
            // at cy+1.
            top: px(footprint.y),
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
            token_sprite_node(unit.position),
            UiTransform::IDENTITY,
            ImageNode::new(ui_assets.map_sprite(unit.archetype).clone()),
            Visibility::Visible,
            ZIndex(depth),
            Pickable::default(),
            ChildOf(stage),
        ))
        .observe(on_battlefield_token_click)
        .observe(on_battlefield_token_move)
        .observe(on_battlefield_token_out)
        .id();
    e2e_id(
        commands,
        card,
        (unit.id == crate::mission::squad::ids::VANGUARD).then_some("battle.unit.vanguard"),
    );

    if unit.faction != Faction::Player {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(4.0),
                top: px(18.0),
                width: px(18.0),
                height: px(18.0),
                ..default()
            },
            theme::icon_node(ui_assets.icons.clone(), style.glyph_rect, style.color),
            Pickable::IGNORE,
            ChildOf(card),
        ));
    }

    let hp_bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(12.0),
                top: px(-10.0),
                width: px(72.0),
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
            width: percent(100.0 * f32::from(unit.hp.max(0)) / f32::from(unit.stats.max_hp.max(1))),
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
            right: px(0.0),
            top: px(-30.0),
            ..default()
        },
        TokenHpText(unit.id),
        Pickable::IGNORE,
        ChildOf(card),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(4.0),
            top: px(4.0),
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

fn token_sprite_node(position: GridPos) -> Node {
    let root = unit_root_top_left(position);
    Node {
        position_type: PositionType::Absolute,
        left: px(root.x),
        top: px(root.y),
        width: px(MAP_UNIT_WIDTH),
        height: px(MAP_UNIT_HEIGHT),
        ..default()
    }
}

fn token_selection_node(position: GridPos) -> Node {
    let top_left = selection_top_left(position);
    Node {
        position_type: PositionType::Absolute,
        left: px(top_left.x),
        top: px(top_left.y),
        width: px(TILE_WIDTH),
        height: px(TILE_HEIGHT),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::battle::BattleState;

    fn blank_ui_assets() -> UiAssets {
        UiAssets {
            key_art: Handle::default(),
            briefing_art: Handle::default(),
            vanguard_art: Handle::default(),
            gunner_art: Handle::default(),
            interceptor_art: Handle::default(),
            vanguard_map: Handle::default(),
            gunner_map: Handle::default(),
            interceptor_map: Handle::default(),
            enemy_map: Handle::default(),
            icons: Handle::default(),
            board: Handle::default(),
            terrain: Handle::default(),
            fonts: std::array::from_fn(|_| Handle::default()),
        }
    }

    #[test]
    fn flat_board_scene_spawns_authored_diamonds_without_map_controls() {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
            .insert_resource(blank_ui_assets())
            .add_systems(Update, setup_mission_scene);
        app.update();

        // The 3x3 fixture board spawns every authored cell and none of the
        // regional chrome.
        let mut cells = app.world_mut().query::<(&CellVisual, &ZIndex, &Pickable)>();
        let cells: Vec<(GridPos, i32, Pickable)> = cells
            .iter(app.world())
            .map(|(cell, z, pickable)| (cell.0, z.0, *pickable))
            .collect();
        assert_eq!(cells.len(), 9);
        assert!(cells.iter().all(|(_, z, _)| *z == 0));
        // Cell outers must stay transparent to picking: a pickable cell would
        // shadow the stage hit and the stage observers would drop the event.
        assert!(
            cells
                .iter()
                .all(|(_, _, pickable)| !pickable.is_hoverable && !pickable.should_block_lower)
        );
        let mut readouts = app
            .world_mut()
            .query::<&crate::presentation::map_view::MapReadout>();
        assert_eq!(readouts.iter(app.world()).count(), 0);

        // Non-regional insets take the authored checkerboard tint.
        let mut insets = app.world_mut().query::<(&CellInsetVisual, &ImageNode)>();
        let mut tint_for = |target: GridPos| {
            insets
                .iter(app.world())
                .find(|(cell, _)| cell.0 == target)
                .map(|(_, image)| image.color)
                .expect("fixture cells spawn insets")
        };
        assert_eq!(tint_for(GridPos::new(0, 0)), theme::BOARD_LIGHT);
        assert_eq!(tint_for(GridPos::new(1, 0)), theme::BOARD_DARK);
    }

    #[test]
    fn rebuild_mission_scene_repaints_flagged_roots_and_clears_the_flag() {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
            .insert_resource(blank_ui_assets())
            .add_systems(Update, rebuild_mission_scene);
        let root = app
            .world_mut()
            .spawn((PresentationRoot, PresentationNeedsRebuild))
            .id();
        app.update();
        assert!(app.world().get::<PresentationNeedsRebuild>(root).is_none());
        let mut stages = app
            .world_mut()
            .query_filtered::<(Entity, &ChildOf), With<BattleStage>>();
        let stage = stages
            .iter(app.world())
            .find(|(_, child)| child.parent() == root)
            .map(|(entity, _)| entity)
            .expect("rebuild spawns a battle stage under the flagged root");
        assert!(app.world().get::<BattleStage>(stage).is_some());
    }
}
