use std::array;

use bevy::prelude::*;

pub const MISSION_ONE_GLTF: &str = "models/mission_one.gltf";
pub const MISSION_ONE_GLTF_DISPLAY_PATH: &str = "assets/models/mission_one.gltf";
pub const MISSION_ONE_SCENE_COUNT: usize = 10;

#[derive(Resource)]
pub struct MissionAssets {
    scenes: [Handle<WorldAsset>; MISSION_ONE_SCENE_COUNT],
}

impl FromWorld for MissionAssets {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            scenes: array::from_fn(|index| {
                asset_server.load(GltfAssetLabel::Scene(index).from_asset(MISSION_ONE_GLTF))
            }),
        }
    }
}

impl MissionAssets {
    pub fn scene(&self, index: usize) -> Handle<WorldAsset> {
        self.scenes[index].clone()
    }

    fn iter(&self) -> impl Iterator<Item = &Handle<WorldAsset>> {
        self.scenes.iter()
    }
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub enum AssetLoadStatus {
    #[default]
    Loading,
    Ready,
    Failed(&'static str),
}

pub fn monitor_mission_assets(
    asset_server: Res<AssetServer>,
    assets: Res<MissionAssets>,
    mut status: ResMut<AssetLoadStatus>,
) {
    if !matches!(*status, AssetLoadStatus::Loading) {
        return;
    }

    let failed = assets.iter().any(|handle| {
        asset_server.load_state(handle.id()).is_failed()
            || asset_server.dependency_load_state(handle.id()).is_failed()
            || asset_server
                .recursive_dependency_load_state(handle.id())
                .is_failed()
    });
    if failed {
        error!("Failed to load {MISSION_ONE_GLTF_DISPLAY_PATH}");
        *status = AssetLoadStatus::Failed(MISSION_ONE_GLTF_DISPLAY_PATH);
        return;
    }

    if assets
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.id()))
    {
        *status = AssetLoadStatus::Ready;
    }
}

pub fn mission_assets_ready(status: Res<AssetLoadStatus>) -> bool {
    matches!(*status, AssetLoadStatus::Ready)
}
