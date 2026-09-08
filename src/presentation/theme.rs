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
pub const PLAYER: Color = Color::srgb_u8(143, 224, 196);
pub const ENEMY: Color = Color::srgb_u8(255, 107, 92);
pub const TEXT: Color = Color::srgb_u8(226, 236, 245);
pub const MUTED: Color = Color::srgb_u8(109, 130, 153);

pub const ICON_ATLAS_SIZE: UVec2 = UVec2::new(512, 512);
pub const ICON_CELL_SIZE: Vec2 = Vec2::splat(64.0);
pub const BOARD_ATLAS_SIZE: UVec2 = UVec2::new(336, 96);

pub const BOARD_DIAMOND_RECT: Rect = Rect::new(0.0, 0.0, 112.0, 56.0);
pub const BOARD_BLOCKER_RECT: Rect = Rect::new(112.0, 0.0, 224.0, 96.0);
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
        assert_eq!(ICON_ATLAS_SIZE, UVec2::new(512, 512));
        assert_eq!(BOARD_ATLAS_SIZE, UVec2::new(336, 96));
        assert_eq!(BOARD_DIAMOND_RECT.size(), Vec2::new(112.0, 56.0));
        assert_eq!(BOARD_BLOCKER_RECT.size(), Vec2::new(112.0, 96.0));
        assert_eq!(BOARD_TOKEN_SHADOW_RECT.size(), Vec2::new(112.0, 96.0));
        assert_eq!(ICON_MOVE.size(), ICON_CELL_SIZE);
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
