//! SRW-style pilot growth: XP earned in campaign battles and the levels it buys.
//!
//! A pilot earns [`TAKEDOWN_XP`] for each enemy it knocks out
//! ([`BOSS_TAKEDOWN_XP`] for a boss) and [`CLEAR_XP`] for every mission the
//! squad wins. Every [`XP_PER_LEVEL`] XP is one level, up to [`MAX_LEVEL`].
//! `mission::squad` projects each level above 1 as +2 accuracy and +2 evasion,
//! and the domain grows the pilot's SP pool from the level (`domain::spirit`).

use serde::{Deserialize, Serialize};

use crate::campaign::model::PlayerMech;
use crate::domain::model::{Takedown, UnitId};
use crate::mission::squad::ids;

pub const TAKEDOWN_XP: u32 = 20;
pub const BOSS_TAKEDOWN_XP: u32 = 60;
pub const CLEAR_XP: u32 = 30;
pub const XP_PER_LEVEL: u32 = 100;
pub const MAX_LEVEL: u8 = 10;
/// Accuracy and evasion each level above 1 adds.
pub const STAT_PER_LEVEL: i16 = 2;

/// The level `xp` buys.
pub fn level_for(xp: u32) -> u8 {
    let level = 1 + xp / XP_PER_LEVEL;
    u8::try_from(level).map_or(MAX_LEVEL, |level| level.min(MAX_LEVEL))
}

/// XP still needed for the next level, or `None` at [`MAX_LEVEL`].
pub fn xp_to_next(xp: u32) -> Option<u32> {
    let level = level_for(xp);
    (level < MAX_LEVEL).then(|| u32::from(level) * XP_PER_LEVEL - xp)
}

/// Accuracy and evasion a pilot of `level` adds to its mech.
pub fn level_stat_bonus(level: u8) -> i16 {
    STAT_PER_LEVEL * i16::from(level.saturating_sub(1))
}

/// Each pilot's lifetime XP.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PilotXp {
    pub vanguard: u32,
    pub gunner: u32,
    pub interceptor: u32,
}

impl PilotXp {
    pub fn xp(&self, mech: PlayerMech) -> u32 {
        match mech {
            PlayerMech::Vanguard => self.vanguard,
            PlayerMech::Gunner => self.gunner,
            PlayerMech::Interceptor => self.interceptor,
        }
    }

    pub fn xp_mut(&mut self, mech: PlayerMech) -> &mut u32 {
        match mech {
            PlayerMech::Vanguard => &mut self.vanguard,
            PlayerMech::Gunner => &mut self.gunner,
            PlayerMech::Interceptor => &mut self.interceptor,
        }
    }

    pub fn level(&self, mech: PlayerMech) -> u8 {
        level_for(self.xp(mech))
    }

    /// Award one won battle's XP and report what each pilot gained.
    pub fn award(&mut self, takedowns: &[Takedown]) -> [PilotGain; 3] {
        PlayerMech::ALL.map(|mech| {
            let credited: Vec<_> = takedowns
                .iter()
                .filter(|takedown| mech_for(takedown.pilot) == Some(mech))
                .collect();
            let takedown_xp: u32 = credited
                .iter()
                .map(|takedown| {
                    if takedown.archetype.is_boss() {
                        BOSS_TAKEDOWN_XP
                    } else {
                        TAKEDOWN_XP
                    }
                })
                .sum();
            let level_before = self.level(mech);
            let gained = CLEAR_XP + takedown_xp;
            let xp = self.xp_mut(mech);
            *xp = xp.saturating_add(gained);
            PilotGain {
                mech,
                takedowns: u8::try_from(credited.len()).unwrap_or(u8::MAX),
                xp_gained: gained,
                xp_after: *xp,
                level_before,
                level_after: level_for(*xp),
            }
        })
    }
}

/// What one pilot earned from a won battle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PilotGain {
    pub mech: PlayerMech,
    pub takedowns: u8,
    pub xp_gained: u32,
    pub xp_after: u32,
    pub level_before: u8,
    pub level_after: u8,
}

impl PilotGain {
    pub fn leveled_up(&self) -> bool {
        self.level_after > self.level_before
    }
}

fn mech_for(unit: UnitId) -> Option<PlayerMech> {
    match unit {
        ids::VANGUARD => Some(PlayerMech::Vanguard),
        ids::GUNNER => Some(PlayerMech::Gunner),
        ids::INTERCEPTOR => Some(PlayerMech::Interceptor),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::UnitArchetype;

    fn takedown(pilot: UnitId, archetype: UnitArchetype) -> Takedown {
        Takedown {
            pilot,
            enemy: UnitId(900),
            archetype,
        }
    }

    #[test]
    fn levels_come_every_hundred_xp_up_to_the_cap() {
        assert_eq!(level_for(0), 1);
        assert_eq!(level_for(99), 1);
        assert_eq!(level_for(100), 2);
        assert_eq!(level_for(950), 10);
        assert_eq!(level_for(u32::MAX), MAX_LEVEL);
        assert_eq!(xp_to_next(0), Some(100));
        assert_eq!(xp_to_next(130), Some(70));
        assert_eq!(xp_to_next(900), None);
        assert_eq!(level_stat_bonus(1), 0);
        assert_eq!(level_stat_bonus(4), 6);
    }

    #[test]
    fn a_win_pays_clear_xp_to_all_and_takedown_xp_to_the_credited_pilot() {
        let mut pilots = PilotXp {
            gunner: 80,
            ..PilotXp::default()
        };
        let gains = pilots.award(&[
            takedown(ids::GUNNER, UnitArchetype::Rifleman),
            takedown(ids::GUNNER, UnitArchetype::Dreadnought),
            takedown(UnitId(77), UnitArchetype::Rifleman),
        ]);
        assert_eq!(pilots.vanguard, CLEAR_XP);
        assert_eq!(pilots.interceptor, CLEAR_XP);
        assert_eq!(
            pilots.gunner,
            80 + CLEAR_XP + TAKEDOWN_XP + BOSS_TAKEDOWN_XP
        );
        let gunner = gains[1];
        assert_eq!(gunner.mech, PlayerMech::Gunner);
        assert_eq!(gunner.takedowns, 2);
        assert_eq!(gunner.xp_gained, 110);
        assert_eq!((gunner.level_before, gunner.level_after), (1, 2));
        assert!(gunner.leveled_up());
        assert!(!gains[0].leveled_up());
        assert_eq!(pilots.level(PlayerMech::Gunner), 2);
    }

    #[test]
    fn an_award_never_wraps_a_maxed_pilot_back_down() {
        let mut pilots = PilotXp {
            vanguard: u32::MAX - 5,
            ..PilotXp::default()
        };
        let gains = pilots.award(&[]);
        assert_eq!(pilots.vanguard, u32::MAX);
        assert_eq!(gains[0].level_after, MAX_LEVEL);
    }
}
