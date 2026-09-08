use bevy::prelude::*;

use super::theme;

pub const KEY_ART_PATH: &str = "ui/key_art.png";
pub const BRIEFING_ART_PATH: &str = "ui/briefing.png";
pub const VANGUARD_ART_PATH: &str = "ui/vanguard.png";
pub const GUNNER_ART_PATH: &str = "ui/gunner.png";
pub const INTERCEPTOR_ART_PATH: &str = "ui/interceptor.png";

/// Handles for every asset used by the native campaign and battle UI.
/// `AssetLoadStatus` is the single readiness/error gate for presentation.
#[derive(Resource)]
pub struct UiAssets {
    pub key_art: Handle<Image>,
    pub briefing_art: Handle<Image>,
    pub vanguard_art: Handle<Image>,
    pub gunner_art: Handle<Image>,
    pub interceptor_art: Handle<Image>,
    pub icons: Handle<Image>,
    pub board: Handle<Image>,
    pub fonts: [Handle<Font>; 7],
}

impl FromWorld for UiAssets {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            key_art: asset_server.load(KEY_ART_PATH),
            briefing_art: asset_server.load(BRIEFING_ART_PATH),
            vanguard_art: asset_server.load(VANGUARD_ART_PATH),
            gunner_art: asset_server.load(GUNNER_ART_PATH),
            interceptor_art: asset_server.load(INTERCEPTOR_ART_PATH),
            icons: asset_server.load(theme::ICON_ATLAS_PATH),
            board: asset_server.load(theme::BOARD_ATLAS_PATH),
            fonts: [
                asset_server.load(theme::CHAKRA_PETCH_400_PATH),
                asset_server.load(theme::CHAKRA_PETCH_500_PATH),
                asset_server.load(theme::CHAKRA_PETCH_600_PATH),
                asset_server.load(theme::CHAKRA_PETCH_700_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_400_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_500_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_600_PATH),
            ],
        }
    }
}

impl UiAssets {
    fn images(&self) -> [(&'static str, &Handle<Image>); 7] {
        [
            (KEY_ART_PATH, &self.key_art),
            (BRIEFING_ART_PATH, &self.briefing_art),
            (VANGUARD_ART_PATH, &self.vanguard_art),
            (GUNNER_ART_PATH, &self.gunner_art),
            (INTERCEPTOR_ART_PATH, &self.interceptor_art),
            (theme::ICON_ATLAS_PATH, &self.icons),
            (theme::BOARD_ATLAS_PATH, &self.board),
        ]
    }

    fn fonts(&self) -> [(&'static str, &Handle<Font>); 7] {
        [
            (theme::CHAKRA_PETCH_400_PATH, &self.fonts[0]),
            (theme::CHAKRA_PETCH_500_PATH, &self.fonts[1]),
            (theme::CHAKRA_PETCH_600_PATH, &self.fonts[2]),
            (theme::CHAKRA_PETCH_700_PATH, &self.fonts[3]),
            (theme::IBM_PLEX_MONO_400_PATH, &self.fonts[4]),
            (theme::IBM_PLEX_MONO_500_PATH, &self.fonts[5]),
            (theme::IBM_PLEX_MONO_600_PATH, &self.fonts[6]),
        ]
    }

    fn failed_path(&self, asset_server: &AssetServer) -> Option<&'static str> {
        self.images()
            .into_iter()
            .find(|(_, handle)| asset_failed(asset_server, handle))
            .map(|(path, _)| path)
            .or_else(|| {
                self.fonts()
                    .into_iter()
                    .find(|(_, handle)| asset_failed(asset_server, handle))
                    .map(|(path, _)| path)
            })
    }

    fn is_ready(&self, asset_server: &AssetServer) -> bool {
        self.images()
            .into_iter()
            .all(|(_, handle)| asset_server.is_loaded_with_dependencies(handle.id()))
            && self
                .fonts()
                .into_iter()
                .all(|(_, handle)| asset_server.is_loaded_with_dependencies(handle.id()))
    }
}

fn asset_failed<T: Asset>(asset_server: &AssetServer, handle: &Handle<T>) -> bool {
    asset_server.load_state(handle.id()).is_failed()
        || asset_server.dependency_load_state(handle.id()).is_failed()
        || asset_server
            .recursive_dependency_load_state(handle.id())
            .is_failed()
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub enum AssetLoadStatus {
    #[default]
    Loading,
    Ready,
    Failed(&'static str),
}

/// Poll the one UI asset catalog. The optional resource keeps small headless
/// fixtures able to run this system without constructing the full app.
pub fn monitor_mission_assets(
    asset_server: Res<AssetServer>,
    ui_assets: Option<Res<UiAssets>>,
    mut status: ResMut<AssetLoadStatus>,
) {
    if !matches!(*status, AssetLoadStatus::Loading) {
        return;
    }
    let Some(ui_assets) = ui_assets else {
        return;
    };
    if let Some(path) = ui_assets.failed_path(&asset_server) {
        error!("Failed to load {path}");
        *status = AssetLoadStatus::Failed(path);
    } else if ui_assets.is_ready(&asset_server) {
        *status = AssetLoadStatus::Ready;
    }
}

pub fn mission_assets_ready(status: &AssetLoadStatus) -> bool {
    matches!(*status, AssetLoadStatus::Ready)
}
