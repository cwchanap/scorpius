use bevy::prelude::*;

pub const MISSION_ONE_GLTF: &str = "models/mission_one.gltf";

pub fn vanguard_scene(asset_server: &AssetServer) -> Handle<WorldAsset> {
    asset_server.load(GltfAssetLabel::Scene(0).from_asset(MISSION_ONE_GLTF))
}
