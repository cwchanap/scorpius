use bevy::prelude::*;

use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{campaign_ui::DialogueCursor, theme};
use super::shared::{ensure_canvas, spawn_dialogue_screen};

pub fn setup_pre_mission_story(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<CampaignRuntime>,
    mut cursor: ResMut<DialogueCursor>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    *cursor = DialogueCursor(0);
    let Some(definition) = super::super::campaign_ui::active_definition(&runtime) else {
        return;
    };
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    spawn_dialogue_screen(
        &mut commands,
        &asset_server,
        &definition.pre_mission,
        super::super::campaign_ui::CampaignUiAction::AdvanceDialogue,
        canvas,
        &ui_assets.fonts,
        &ui_assets.icons,
        theme::ACCENT,
        true,
    );
}
