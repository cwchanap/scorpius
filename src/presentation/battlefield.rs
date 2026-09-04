use std::f32::consts::PI;

use bevy::{camera::ScalingMode, prelude::*};

use crate::domain::{
    board::GridPos,
    model::{Faction, Reaction, UnitArchetype},
};

use super::{
    BattleRuntime, CellVisual, PresentationNeedsRebuild, PresentationRoot, PropVisual, UnitVisual,
    assets::MissionAssets,
    grid_to_world,
    interaction::{on_battlefield_cell_click, on_battlefield_cell_out, on_battlefield_cell_over},
};

#[derive(Resource)]
pub struct BattlefieldVisualAssets {
    pub tile_mesh: Handle<Mesh>,
    pub telegraph_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub glyph_bar_mesh: Handle<Mesh>,
    pub line_mesh: Handle<Mesh>,
    pub tile_light: Handle<StandardMaterial>,
    pub tile_dark: Handle<StandardMaterial>,
    pub tile_selected: Handle<StandardMaterial>,
    pub tile_reachable: Handle<StandardMaterial>,
    pub tile_attack_preview: Handle<StandardMaterial>,
    pub telegraph: Handle<StandardMaterial>,
    pub telegraph_edge: Handle<StandardMaterial>,
    pub intended_target: Handle<StandardMaterial>,
    pub intent_line: Handle<StandardMaterial>,
    pub guard: Handle<StandardMaterial>,
    pub evade: Handle<StandardMaterial>,
    pub counter: Handle<StandardMaterial>,
}

impl BattlefieldVisualAssets {
    pub fn reaction_material(&self, reaction: Reaction) -> Handle<StandardMaterial> {
        match reaction {
            Reaction::Guard => self.guard.clone(),
            Reaction::Evade => self.evade.clone(),
            Reaction::Counter => self.counter.clone(),
        }
    }
}

pub fn mission_grid_cells(width: u8, height: u8) -> Vec<GridPos> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| GridPos::new(x, y)))
        .collect()
}

pub const fn scene_index(archetype: UnitArchetype) -> usize {
    match archetype {
        UnitArchetype::Vanguard => 0,
        UnitArchetype::Gunner => 1,
        UnitArchetype::Interceptor => 2,
        UnitArchetype::Rifleman => 3,
        UnitArchetype::Striker => 4,
        UnitArchetype::Artillery => 5,
        UnitArchetype::Flanker => 10,
        UnitArchetype::Bulwark => 11,
        UnitArchetype::Controller => 12,
        UnitArchetype::Dreadnought | UnitArchetype::Regent => 13,
    }
}

pub fn setup_mission_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mission_assets: Res<MissionAssets>,
    battle: Res<BattleRuntime>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.28, 0.38, 0.52),
        brightness: 260.0,
        affects_lightmapped_meshes: true,
    });
    commands.spawn((
        Camera3d::default(),
        MeshPickingCamera,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 12.8,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(10.8, 12.4, 12.2).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.95, -0.55, 0.0)),
    ));

    let visual_assets = create_visual_assets(&mut meshes, &mut materials);
    let root = spawn_presentation_root(&mut commands);
    populate_mission_root(
        &mut commands,
        root,
        &mission_assets,
        &battle,
        &visual_assets,
    );
    commands.insert_resource(visual_assets);
}

pub(crate) fn rebuild_mission_scene(
    mut commands: Commands,
    mission_assets: Res<MissionAssets>,
    battle: Res<BattleRuntime>,
    visual_assets: Res<BattlefieldVisualAssets>,
    roots: Query<Entity, (With<PresentationRoot>, With<PresentationNeedsRebuild>)>,
) {
    for root in &roots {
        populate_mission_root(
            &mut commands,
            root,
            &mission_assets,
            &battle,
            &visual_assets,
        );
        commands.entity(root).remove::<PresentationNeedsRebuild>();
    }
}

fn spawn_presentation_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Name::new("Mission 1 Presentation"),
            PresentationRoot,
            Transform::default(),
            Visibility::Visible,
        ))
        .id()
}

fn populate_mission_root(
    commands: &mut Commands,
    root: Entity,
    mission_assets: &MissionAssets,
    battle: &BattleRuntime,
    visual_assets: &BattlefieldVisualAssets,
) {
    for cell in mission_grid_cells(battle.0.board().width(), battle.0.board().height()) {
        let material = if (cell.x + cell.y) % 2 == 0 {
            visual_assets.tile_light.clone()
        } else {
            visual_assets.tile_dark.clone()
        };
        commands
            .spawn((
                Name::new(format!("Cell {},{}", cell.x, cell.y)),
                Mesh3d(visual_assets.tile_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(grid_to_world(cell) - Vec3::Y * 0.2),
                CellVisual(cell),
                Pickable::default(),
                ChildOf(root),
            ))
            .observe(on_battlefield_cell_click)
            .observe(on_battlefield_cell_over)
            .observe(on_battlefield_cell_out);
    }

    for unit in battle.0.units() {
        let rotation = if unit.faction == Faction::Enemy {
            Quat::from_rotation_y(PI)
        } else {
            Quat::IDENTITY
        };
        commands.spawn((
            Name::new(unit.name),
            WorldAssetRoot(mission_assets.scene(scene_index(unit.archetype))),
            Transform::from_translation(grid_to_world(unit.position))
                .with_rotation(rotation)
                .with_scale(Vec3::splat(0.72)),
            Visibility::Visible,
            UnitVisual(unit.id),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }

    for cell in battle.0.board().blocking_cells() {
        commands.spawn((
            Name::new(format!("Blocking {},{}", cell.x, cell.y)),
            WorldAssetRoot(mission_assets.scene(6)),
            Transform::from_translation(grid_to_world(cell)).with_scale(Vec3::splat(0.82)),
            Visibility::Visible,
            PropVisual::Blocking(cell),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    for explosive in battle.0.board().explosives() {
        commands.spawn((
            Name::new(format!(
                "Explosive {},{}",
                explosive.position.x, explosive.position.y
            )),
            WorldAssetRoot(mission_assets.scene(7)),
            Transform::from_translation(grid_to_world(explosive.position))
                .with_scale(Vec3::splat(0.78)),
            if explosive.exploded {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
            PropVisual::Explosive(explosive.position),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    for cell in battle.0.board().hazard_cells() {
        commands.spawn((
            Name::new(format!("Hazard {},{}", cell.x, cell.y)),
            WorldAssetRoot(mission_assets.scene(8)),
            Transform::from_translation(grid_to_world(cell) + Vec3::Y * 0.01)
                .with_scale(Vec3::splat(0.86)),
            Visibility::Visible,
            PropVisual::Hazard(cell),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
}

pub fn create_visual_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> BattlefieldVisualAssets {
    let deck_material = |color: Color, roughness: f32| StandardMaterial {
        base_color: color,
        metallic: 0.68,
        perceptual_roughness: roughness,
        ..default()
    };
    let unlit_translucent = |color: Color| StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    };

    BattlefieldVisualAssets {
        tile_mesh: meshes.add(Cuboid::new(0.94, 0.12, 0.94)),
        telegraph_mesh: meshes.add(Cuboid::new(0.86, 0.035, 0.86)),
        ring_mesh: meshes.add(Torus::new(0.34, 0.43)),
        glyph_bar_mesh: meshes.add(Cuboid::new(0.62, 0.025, 0.075)),
        line_mesh: meshes.add(Cuboid::new(1.0, 0.028, 0.055)),
        tile_light: materials.add(deck_material(Color::srgb(0.13, 0.19, 0.25), 0.7)),
        tile_dark: materials.add(deck_material(Color::srgb(0.055, 0.085, 0.13), 0.78)),
        tile_selected: materials.add(deck_material(Color::srgb(0.08, 0.64, 0.78), 0.55)),
        tile_reachable: materials.add(deck_material(Color::srgb(0.08, 0.34, 0.43), 0.62)),
        tile_attack_preview: materials.add(deck_material(Color::srgb(0.94, 0.5, 0.08), 0.5)),
        telegraph: materials.add(unlit_translucent(Color::srgba(0.96, 0.08, 0.12, 0.34))),
        telegraph_edge: materials.add(unlit_translucent(Color::srgba(1.0, 0.26, 0.18, 0.88))),
        intended_target: materials.add(unlit_translucent(Color::srgba(1.0, 1.0, 1.0, 0.92))),
        intent_line: materials.add(unlit_translucent(Color::srgba(1.0, 0.08, 0.12, 0.78))),
        guard: materials.add(unlit_translucent(Color::srgba(0.18, 0.9, 0.42, 0.9))),
        evade: materials.add(unlit_translucent(Color::srgba(0.16, 0.58, 1.0, 0.9))),
        counter: materials.add(unlit_translucent(Color::srgba(1.0, 0.46, 0.08, 0.9))),
    }
}
