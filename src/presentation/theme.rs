use bevy::prelude::*;

use crate::domain::model::UnitArchetype;

pub const ICON_ATLAS_PATH: &str = "ui/icons.png";
pub const BOARD_ATLAS_PATH: &str = "ui/board.png";

pub const CHAKRA_PETCH_400_PATH: &str = "fonts/chakra-petch-400.ttf";
pub const CHAKRA_PETCH_500_PATH: &str = "fonts/chakra-petch-500.ttf";
pub const CHAKRA_PETCH_600_PATH: &str = "fonts/chakra-petch-600.ttf";
pub const CHAKRA_PETCH_700_PATH: &str = "fonts/chakra-petch-700.ttf";
pub const IBM_PLEX_MONO_400_PATH: &str = "fonts/ibm-plex-mono-400.ttf";
pub const IBM_PLEX_MONO_500_PATH: &str = "fonts/ibm-plex-mono-500.ttf";
pub const IBM_PLEX_MONO_600_PATH: &str = "fonts/ibm-plex-mono-600.ttf";

pub type FontHandles = [Handle<Font>; 7];

pub const BACKGROUND: Color = Color::srgb_u8(5, 8, 15);
pub const PANEL: Color = Color::srgb_u8(10, 20, 32);
pub const PANEL_RAISED: Color = Color::srgb_u8(14, 44, 58);
pub const BORDER: Color = Color::srgb_u8(29, 50, 68);
pub const ACCENT: Color = Color::srgb_u8(62, 199, 219);
pub const GOLD: Color = Color::srgb_u8(255, 209, 117);
pub const MINT: Color = Color::srgb_u8(127, 220, 154);
pub const RESULT_CARD_BACKGROUND: Color = Color::srgb_u8(7, 13, 22);
pub const RESULT_VICTORY_BORDER: Color = Color::srgb_u8(44, 90, 66);
pub const RESULT_DEFEAT_BORDER: Color = Color::srgb_u8(74, 43, 34);
pub const RESULT_VICTORY_TEXT: Color = Color::srgb_u8(201, 242, 216);
pub const RESULT_DEFEAT_TEXT: Color = Color::srgb_u8(255, 201, 194);
pub const PLAYER: Color = Color::srgb_u8(143, 224, 196);
pub const ENEMY: Color = Color::srgb_u8(255, 107, 92);
pub const TEXT: Color = Color::srgb_u8(226, 236, 245);
pub const MUTED: Color = Color::srgb_u8(109, 130, 153);
pub const BOARD_STROKE: Color = Color::srgb_u8(22, 40, 58);
pub const BOARD_LIGHT: Color = Color::srgb_u8(13, 24, 38);
pub const BOARD_DARK: Color = Color::srgb_u8(10, 20, 32);
pub const BOARD_REACHABLE: Color = Color::srgb_u8(20, 87, 108);
pub const BOARD_SELECTED: Color = Color::srgb_u8(62, 199, 219);
pub const BOARD_ATTACK: Color = Color::srgb_u8(255, 209, 117);
pub const BOARD_ATTACK_INSET: Color = Color::srgb_u8(82, 64, 31);
pub const BOARD_INSPECTED: Color = Color::srgb_u8(255, 156, 96);
pub const BOARD_TELEGRAPH: Color = Color::srgba(1.0, 0.12, 0.1, 0.56);
pub const BOARD_EXTRACTION: Color = Color::srgba(1.0, 0.82, 0.38, 0.75);
pub const BOARD_HAZARD: Color = Color::srgba(1.0, 0.2, 0.14, 0.72);
pub const BOARD_EXPLOSIVE: Color = Color::srgba(1.0, 0.62, 0.2, 0.9);

pub const ICON_ATLAS_SIZE: UVec2 = UVec2::new(512, 576);
pub const ICON_CELL_SIZE: Vec2 = Vec2::splat(64.0);
pub const BOARD_ATLAS_SIZE: UVec2 = UVec2::new(336, 96);

pub const BOARD_DIAMOND_RECT: Rect = Rect::new(0.0, 0.0, 112.0, 56.0);
// The prism's visible geometry ends at TILE_HEIGHT / 2 + BLOCK_HEIGHT = 82px;
// crop the transparent atlas tail so the UI node keeps the source dimensions.
pub const BOARD_BLOCKER_RECT: Rect = Rect::new(112.0, 0.0, 224.0, 82.0);
pub const BOARD_TOKEN_SHADOW_RECT: Rect = Rect::new(224.0, 0.0, 336.0, 96.0);

/// The source atlas is packed in 64px cells. Keeping this conversion const
/// leaves the atlas mapping in Rust instead of making runtime geometry data.
pub const fn icon_rect(slot: u32) -> Rect {
    let x = (slot % 8) as f32 * ICON_CELL_SIZE.x;
    let y = (slot / 8) as f32 * ICON_CELL_SIZE.y;
    Rect::new(x, y, x + ICON_CELL_SIZE.x, y + ICON_CELL_SIZE.y)
}

pub const UNIT_GLYPH_HEX_RECT: Rect = icon_rect(51);
pub const UNIT_GLYPH_DIAMOND_RECT: Rect = icon_rect(52);
pub const UNIT_GLYPH_TRIANGLE_RECT: Rect = icon_rect(53);
pub const UNIT_GLYPH_SQUARE_RECT: Rect = icon_rect(54);

// These eight masks are the source D.* actions rendered into the previously
// unused atlas cells. Keeping semantic actions on white masks lets ImageNode
// apply the source palette at the call site without relying on an unrelated
// pre-colored SVG variant.
pub const ICON_MOVE: Rect = icon_rect(55);
pub const ICON_ATTACK: Rect = icon_rect(56);
pub const ICON_GUARD: Rect = icon_rect(57);
pub const ICON_COUNTER: Rect = icon_rect(58);
pub const ICON_EVADE: Rect = icon_rect(59);
pub const ICON_SKILL: Rect = icon_rect(60);
pub const ICON_WAIT: Rect = icon_rect(61);
pub const ICON_BACK: Rect = icon_rect(62);

// These crops remove the 64px exporter padding from the source-colored
// campaign glyphs. The masks above keep their full cells because they are
// tinted by the semantic action that owns them.
pub const TITLE_EMBLEM_RECT: Rect = Rect::new(4.0, 4.0, 60.0, 60.0);
pub const TITLE_ORNAMENT_GRID_RECT: Rect = Rect::new(83.0, 19.0, 109.0, 45.0);
pub const TITLE_ORNAMENT_CIRCLE_RECT: Rect = Rect::new(147.0, 19.0, 173.0, 45.0);
pub const TITLE_ORNAMENT_DIAGONAL_RECT: Rect = Rect::new(211.0, 19.0, 237.0, 45.0);
pub const TITLE_NEW_GAME_RECT: Rect = Rect::new(268.0, 12.0, 308.0, 52.0);
pub const TITLE_CONTINUE_RECT: Rect = Rect::new(332.0, 12.0, 372.0, 52.0);
pub const BRIEFING_GRID_RECT: Rect = Rect::new(84.0, 84.0, 108.0, 108.0);
pub const BRIEFING_ENEMY_RECT: Rect = Rect::new(148.0, 84.0, 172.0, 108.0);
pub const BRIEFING_HAZARD_RECT: Rect = Rect::new(212.0, 84.0, 236.0, 108.0);
pub const BRIEFING_PRIMARY_RECT: Rect = Rect::new(265.0, 73.0, 311.0, 119.0);
pub const BRIEFING_BONUS_RECT: Rect = Rect::new(329.0, 73.0, 375.0, 119.0);
pub const BRIEFING_BASE_REWARD_RECT: Rect = Rect::new(406.0, 86.0, 426.0, 106.0);
pub const BRIEFING_BONUS_REWARD_RECT: Rect = Rect::new(470.0, 86.0, 490.0, 106.0);
pub const BRIEFING_DEPLOY_RECT: Rect = Rect::new(12.0, 140.0, 52.0, 180.0);
pub const BATTLE_ALLY_RECT: Rect = Rect::new(84.0, 148.0, 108.0, 172.0);
pub const BATTLE_ENEMY_RECT: Rect = Rect::new(148.0, 148.0, 172.0, 172.0);
pub const BATTLE_AWAITING_RECT: Rect = Rect::new(212.0, 148.0, 236.0, 172.0);
pub const BATTLE_CYCLE_RECT: Rect = Rect::new(276.0, 148.0, 300.0, 172.0);
pub const BATTLE_PRIMARY_RECT: Rect = Rect::new(340.0, 148.0, 364.0, 172.0);
pub const BATTLE_CREDITS_RECT: Rect = Rect::new(405.0, 149.0, 427.0, 171.0);
pub const BATTLE_RESTART_RECT: Rect = Rect::new(469.0, 149.0, 493.0, 173.0);
pub const RESULT_PRIMARY_RECT: Rect = Rect::new(276.0, 340.0, 300.0, 364.0);
pub const RESULT_BONUS_RECT: Rect = Rect::new(340.0, 340.0, 364.0, 364.0);
pub const RESULT_VICTORY_RECT: Rect = Rect::new(132.0, 324.0, 188.0, 380.0);
pub const RESULT_DEFEAT_RECT: Rect = Rect::new(196.0, 324.0, 252.0, 380.0);
pub const HANGAR_RECT: Rect = Rect::new(459.0, 331.0, 501.0, 373.0);
pub const CREDITS_LARGE_RECT: Rect = Rect::new(17.0, 81.0, 47.0, 111.0);
pub const CREDITS_SMALL_RECT: Rect = Rect::new(403.0, 339.0, 429.0, 365.0);
pub const ENDING_EMBLEM_RECT: Rect = Rect::new(68.0, 388.0, 124.0, 444.0);
pub const ENDING_RETURN_RECT: Rect = Rect::new(143.0, 399.0, 177.0, 433.0);

// Battle sidebar crops retain the authored source boxes instead of resizing
// an entire 64px exporter cell down to the requested UI size.
pub const INSPECTOR_HP_RECT: Rect = Rect::new(24.0, 216.0, 40.0, 232.0);
pub const INSPECTOR_EN_RECT: Rect = Rect::new(88.0, 216.0, 104.0, 232.0);
pub const INSPECTOR_ARMOR_RECT: Rect = Rect::new(151.5, 215.5, 168.5, 232.5);
pub const INSPECTOR_MOBILITY_RECT: Rect = Rect::new(215.5, 215.5, 232.5, 232.5);
pub const INSPECTOR_EVASION_RECT: Rect = Rect::new(279.5, 215.5, 296.5, 232.5);
pub const EMPTY_INSPECTOR_RECT: Rect = Rect::new(331.0, 203.0, 373.0, 245.0);
pub const MENU_BACK_RECT: Rect = Rect::new(406.0, 214.0, 422.0, 230.0);
pub const MENU_TARGET_RECT: Rect = Rect::new(470.0, 214.0, 486.0, 230.0);
pub const STANCE_GUARD_RECT: Rect = Rect::new(22.0, 278.0, 46.0, 302.0);
pub const TARGETING_CANCEL_RECT: Rect = Rect::new(83.0, 275.0, 109.0, 301.0);
pub const LOG_RECT: Rect = Rect::new(151.0, 279.0, 169.0, 297.0);
pub const LOCKED_RECT: Rect = Rect::new(276.0, 276.0, 300.0, 300.0);
pub const PREVIEW_RECT: Rect = Rect::new(343.0, 279.0, 361.0, 297.0);
pub const PREVIEW_HIT_RECT: Rect = Rect::new(407.0, 279.0, 425.0, 297.0);

// These source-defined track and purchase masks are rendered from the
// existing 24px icon() paths into the previously empty slot 63 and one
// appended atlas row. Their cropped rects keep the source SVG viewBox size so
// ImageNode tinting remains exact for normal, maxed, and disabled states.
pub const TRACK_HP_RECT: Rect = Rect::new(468.0, 468.0, 492.0, 492.0);
pub const TRACK_MOBILITY_HANGAR_RECT: Rect = Rect::new(20.0, 532.0, 44.0, 556.0);
pub const TRACK_MOBILITY_ENDING_RECT: Rect = Rect::new(84.0, 532.0, 108.0, 556.0);
pub const TRACK_WEAPON_RECT: Rect = Rect::new(148.0, 532.0, 172.0, 556.0);
pub const CREDITS_PURCHASE_RECT: Rect = Rect::new(212.0, 532.0, 236.0, 556.0);

// Existing dialogue and hangar arrows retain the source-sized content boxes.
pub const ICON_FORWARD: Rect = Rect::new(390.0, 6.0, 442.0, 58.0);
pub const ICON_SKIP: Rect = Rect::new(468.0, 20.0, 492.0, 44.0);
pub const ICON_FORWARD_COMPACT: Rect = Rect::new(14.0, 398.0, 50.0, 434.0);
pub const THREAT_ARROW_RECT: Rect = Rect::new(469.0, 277.0, 491.0, 299.0);
pub const THREAT_DAMAGE_RECT: Rect = Rect::new(22.0, 342.0, 42.0, 362.0);
pub const THREAT_HIT_RECT: Rect = Rect::new(87.5, 343.5, 104.5, 360.5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontFamily {
    ChakraPetch,
    IbmPlexMono,
}

pub fn text_font(
    fonts: &FontHandles,
    family: FontFamily,
    size: f32,
    weight: FontWeight,
) -> TextFont {
    TextFont {
        font: FontSource::Handle(fonts[font_index(family, weight)].clone()),
        font_size: FontSize::Px(size),
        weight,
        ..default()
    }
}

pub fn chakra_petch(fonts: &FontHandles, size: f32, weight: FontWeight) -> TextFont {
    text_font(fonts, FontFamily::ChakraPetch, size, weight)
}

pub fn ibm_plex_mono(fonts: &FontHandles, size: f32, weight: FontWeight) -> TextFont {
    text_font(fonts, FontFamily::IbmPlexMono, size, weight)
}

const fn font_index(family: FontFamily, weight: FontWeight) -> usize {
    match (family, weight.0) {
        (FontFamily::ChakraPetch, 400) => 0,
        (FontFamily::ChakraPetch, 500) => 1,
        (FontFamily::ChakraPetch, 600) => 2,
        (FontFamily::ChakraPetch, 700) => 3,
        (FontFamily::IbmPlexMono, 400) => 4,
        (FontFamily::IbmPlexMono, 500) => 5,
        (FontFamily::IbmPlexMono, 600) => 6,
        _ => panic!("unsupported HPA-480 font face; add the exact bundled asset"),
    }
}

pub fn icon_node(image: Handle<Image>, rect: Rect, color: Color) -> ImageNode {
    ImageNode::new(image).with_rect(rect).with_color(color)
}

pub fn board_node(image: Handle<Image>, rect: Rect, color: Color) -> ImageNode {
    ImageNode::new(image).with_rect(rect).with_color(color)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitArchetypeStyle {
    pub glyph_rect: Rect,
    pub footprint_rect: Rect,
    pub color: Color,
}

/// Exhaustive presentation mapping for every domain archetype. The board
/// token footprint is shared geometry; glyph shape and archetype colors stay
/// data in this match so no parallel `UnitGlyph` type or fallback can drift.
pub const fn unit_archetype_style(archetype: UnitArchetype) -> UnitArchetypeStyle {
    // The first six mappings reproduce the source glyph() contract exactly:
    // Vanguard hex, Gunner diamond, Interceptor triangle, Rifleman square,
    // Striker triangle, and Artillery diamond. Later archetypes reuse those
    // shapes explicitly until their Task 3 extension review.
    let (glyph_rect, color) = match archetype {
        UnitArchetype::Vanguard => (UNIT_GLYPH_HEX_RECT, ACCENT),
        UnitArchetype::Gunner => (UNIT_GLYPH_DIAMOND_RECT, Color::srgb_u8(127, 191, 232)),
        UnitArchetype::Interceptor => (UNIT_GLYPH_TRIANGLE_RECT, PLAYER),
        UnitArchetype::Rifleman => (UNIT_GLYPH_SQUARE_RECT, Color::srgb_u8(255, 143, 128)),
        UnitArchetype::Striker => (UNIT_GLYPH_TRIANGLE_RECT, ENEMY),
        UnitArchetype::Artillery => (UNIT_GLYPH_DIAMOND_RECT, Color::srgb_u8(255, 176, 76)),
        UnitArchetype::Flanker => (UNIT_GLYPH_TRIANGLE_RECT, Color::srgb_u8(180, 140, 255)),
        UnitArchetype::Bulwark => (UNIT_GLYPH_SQUARE_RECT, GOLD),
        UnitArchetype::Controller => (UNIT_GLYPH_DIAMOND_RECT, Color::srgb_u8(95, 200, 232)),
        UnitArchetype::Dreadnought => (UNIT_GLYPH_HEX_RECT, Color::srgb_u8(220, 68, 80)),
        UnitArchetype::Regent => (UNIT_GLYPH_HEX_RECT, Color::srgb_u8(180, 140, 255)),
    };
    UnitArchetypeStyle {
        glyph_rect,
        footprint_rect: BOARD_TOKEN_SHADOW_RECT,
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_and_atlas_contract_is_source_shaped() {
        assert_eq!(ACCENT, Color::srgb_u8(0x3e, 0xc7, 0xdb));
        assert_eq!(GOLD, Color::srgb_u8(0xff, 0xd1, 0x75));
        assert_eq!(ICON_CELL_SIZE, Vec2::splat(64.0));
        assert_eq!(ICON_ATLAS_SIZE, UVec2::new(512, 576));
        assert_eq!(BOARD_ATLAS_SIZE, UVec2::new(336, 96));
        assert_eq!(BOARD_DIAMOND_RECT.size(), Vec2::new(112.0, 56.0));
        assert_eq!(BOARD_BLOCKER_RECT.size(), Vec2::new(112.0, 82.0));
        assert_eq!(BOARD_TOKEN_SHADOW_RECT.size(), Vec2::new(112.0, 96.0));
        assert_eq!(ICON_MOVE.size(), ICON_CELL_SIZE);
        assert_eq!(TITLE_EMBLEM_RECT.size(), Vec2::splat(56.0));
        assert_eq!(TITLE_ORNAMENT_GRID_RECT.size(), Vec2::splat(26.0));
        assert_eq!(TITLE_NEW_GAME_RECT.size(), Vec2::splat(40.0));
        assert_eq!(BRIEFING_PRIMARY_RECT.size(), Vec2::splat(46.0));
        assert_eq!(BATTLE_CREDITS_RECT.size(), Vec2::splat(22.0));
        assert_eq!(ENDING_EMBLEM_RECT.size(), Vec2::splat(56.0));
        assert_eq!(TRACK_HP_RECT.size(), Vec2::splat(24.0));
        assert_eq!(TRACK_MOBILITY_HANGAR_RECT.size(), Vec2::splat(24.0));
        assert_eq!(TRACK_MOBILITY_ENDING_RECT.size(), Vec2::splat(24.0));
        assert_eq!(TRACK_WEAPON_RECT.size(), Vec2::splat(24.0));
        assert_eq!(CREDITS_PURCHASE_RECT.size(), Vec2::splat(24.0));
        assert_eq!(INSPECTOR_HP_RECT.size(), Vec2::splat(16.0));
        assert_eq!(INSPECTOR_EN_RECT.size(), Vec2::splat(16.0));
        assert_eq!(INSPECTOR_ARMOR_RECT.size(), Vec2::splat(17.0));
        assert_eq!(INSPECTOR_MOBILITY_RECT.size(), Vec2::splat(17.0));
        assert_eq!(INSPECTOR_EVASION_RECT.size(), Vec2::splat(17.0));
        assert_eq!(EMPTY_INSPECTOR_RECT.size(), Vec2::new(42.0, 42.0));
        assert_eq!(MENU_BACK_RECT.size(), Vec2::splat(16.0));
        assert_eq!(MENU_TARGET_RECT.size(), Vec2::splat(16.0));
        assert_eq!(STANCE_GUARD_RECT.size(), Vec2::splat(24.0));
        assert_eq!(TARGETING_CANCEL_RECT.size(), Vec2::splat(26.0));
        assert_eq!(LOG_RECT.size(), Vec2::splat(18.0));
        assert_eq!(LOCKED_RECT.size(), Vec2::splat(24.0));
        assert_eq!(PREVIEW_RECT.size(), Vec2::splat(18.0));
        assert_eq!(PREVIEW_HIT_RECT.size(), Vec2::splat(18.0));
        assert_eq!(THREAT_ARROW_RECT.size(), Vec2::splat(22.0));
        assert_eq!(THREAT_DAMAGE_RECT.size(), Vec2::splat(20.0));
        assert_eq!(THREAT_HIT_RECT.size(), Vec2::splat(17.0));
        for rect in [
            UNIT_GLYPH_HEX_RECT,
            UNIT_GLYPH_DIAMOND_RECT,
            UNIT_GLYPH_TRIANGLE_RECT,
            UNIT_GLYPH_SQUARE_RECT,
            ICON_MOVE,
            ICON_ATTACK,
            ICON_GUARD,
            ICON_COUNTER,
            ICON_EVADE,
            ICON_SKILL,
            ICON_WAIT,
            ICON_BACK,
            ICON_SKIP,
            ICON_FORWARD,
            ICON_FORWARD_COMPACT,
            TITLE_EMBLEM_RECT,
            TITLE_ORNAMENT_GRID_RECT,
            TITLE_ORNAMENT_CIRCLE_RECT,
            TITLE_ORNAMENT_DIAGONAL_RECT,
            TITLE_NEW_GAME_RECT,
            TITLE_CONTINUE_RECT,
            BRIEFING_GRID_RECT,
            BRIEFING_ENEMY_RECT,
            BRIEFING_HAZARD_RECT,
            BRIEFING_PRIMARY_RECT,
            BRIEFING_BONUS_RECT,
            BRIEFING_BASE_REWARD_RECT,
            BRIEFING_BONUS_REWARD_RECT,
            BRIEFING_DEPLOY_RECT,
            BATTLE_ALLY_RECT,
            BATTLE_ENEMY_RECT,
            BATTLE_AWAITING_RECT,
            BATTLE_CYCLE_RECT,
            BATTLE_PRIMARY_RECT,
            BATTLE_CREDITS_RECT,
            BATTLE_RESTART_RECT,
            RESULT_PRIMARY_RECT,
            RESULT_BONUS_RECT,
            RESULT_VICTORY_RECT,
            RESULT_DEFEAT_RECT,
            HANGAR_RECT,
            CREDITS_LARGE_RECT,
            CREDITS_SMALL_RECT,
            ENDING_EMBLEM_RECT,
            ENDING_RETURN_RECT,
            TRACK_HP_RECT,
            TRACK_MOBILITY_HANGAR_RECT,
            TRACK_MOBILITY_ENDING_RECT,
            TRACK_WEAPON_RECT,
            CREDITS_PURCHASE_RECT,
            INSPECTOR_HP_RECT,
            INSPECTOR_EN_RECT,
            INSPECTOR_ARMOR_RECT,
            INSPECTOR_MOBILITY_RECT,
            INSPECTOR_EVASION_RECT,
            EMPTY_INSPECTOR_RECT,
            MENU_BACK_RECT,
            MENU_TARGET_RECT,
            STANCE_GUARD_RECT,
            TARGETING_CANCEL_RECT,
            LOG_RECT,
            LOCKED_RECT,
            PREVIEW_RECT,
            PREVIEW_HIT_RECT,
            THREAT_ARROW_RECT,
            THREAT_DAMAGE_RECT,
            THREAT_HIT_RECT,
        ] {
            assert!(rect.max.x <= ICON_ATLAS_SIZE.x as f32);
            assert!(rect.max.y <= ICON_ATLAS_SIZE.y as f32);
        }
    }

    #[test]
    fn canonical_archetype_glyphs_and_colors_are_explicit() {
        let styles = [
            (UnitArchetype::Vanguard, UNIT_GLYPH_HEX_RECT, ACCENT),
            (
                UnitArchetype::Gunner,
                UNIT_GLYPH_DIAMOND_RECT,
                Color::srgb_u8(127, 191, 232),
            ),
            (UnitArchetype::Interceptor, UNIT_GLYPH_TRIANGLE_RECT, PLAYER),
            (
                UnitArchetype::Rifleman,
                UNIT_GLYPH_SQUARE_RECT,
                Color::srgb_u8(255, 143, 128),
            ),
            (UnitArchetype::Striker, UNIT_GLYPH_TRIANGLE_RECT, ENEMY),
            (
                UnitArchetype::Artillery,
                UNIT_GLYPH_DIAMOND_RECT,
                Color::srgb_u8(255, 176, 76),
            ),
            (
                UnitArchetype::Flanker,
                UNIT_GLYPH_TRIANGLE_RECT,
                Color::srgb_u8(180, 140, 255),
            ),
            (UnitArchetype::Bulwark, UNIT_GLYPH_SQUARE_RECT, GOLD),
            (
                UnitArchetype::Controller,
                UNIT_GLYPH_DIAMOND_RECT,
                Color::srgb_u8(95, 200, 232),
            ),
            (
                UnitArchetype::Dreadnought,
                UNIT_GLYPH_HEX_RECT,
                Color::srgb_u8(220, 68, 80),
            ),
            (
                UnitArchetype::Regent,
                UNIT_GLYPH_HEX_RECT,
                Color::srgb_u8(180, 140, 255),
            ),
        ];
        for (archetype, glyph_rect, color) in styles {
            let style = unit_archetype_style(archetype);
            assert_eq!(style.glyph_rect, glyph_rect);
            assert_eq!(style.footprint_rect, BOARD_TOKEN_SHADOW_RECT);
            assert_eq!(style.color, color);
            assert_ne!(style.glyph_rect, style.footprint_rect);
        }
    }
}
