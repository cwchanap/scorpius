//! Shared fixed enemy roster construction.
//!
//! Missions own their deployment positions and opening plans; this module owns
//! the enemy units themselves so every mission projects the same locked stats
//! and weapons the same way.

use crate::domain::board::GridPos;
use crate::domain::model::{Faction, UnitArchetype, UnitId, UnitState, WeaponShape, WeaponSpec};
use crate::mission::squad::{stats, unit, weapon};

pub mod ids {
    use crate::domain::model::WeaponId;

    pub const SERVICE_RIFLE: WeaponId = WeaponId(201);
    pub const SHOCK_CLAW: WeaponId = WeaponId(202);
    pub const SIEGE_MORTAR: WeaponId = WeaponId(203);
    pub const SKIRMISH_CARBINE: WeaponId = WeaponId(204);
}

pub fn rifleman(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Rifleman,
        Faction::Enemy,
        stats(9, 1, 2, 72, 5, 0),
        position,
        vec![ids::SERVICE_RIFLE],
    )
}

pub fn striker(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Striker,
        Faction::Enemy,
        stats(12, 2, 2, 78, 10, 0),
        position,
        vec![ids::SHOCK_CLAW],
    )
}

pub fn artillery(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Artillery,
        Faction::Enemy,
        stats(10, 1, 1, 90, 0, 0),
        position,
        vec![ids::SIEGE_MORTAR],
    )
}

pub fn flanker(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Flanker,
        Faction::Enemy,
        stats(8, 0, 4, 82, 30, 0),
        position,
        vec![ids::SKIRMISH_CARBINE],
    )
}

pub const fn service_rifle() -> WeaponSpec {
    weapon(
        ids::SERVICE_RIFLE,
        "Service Rifle",
        2,
        4,
        WeaponShape::Single,
        5,
        0,
        5,
        0,
        false,
        false,
    )
}

pub const fn shock_claw() -> WeaponSpec {
    weapon(
        ids::SHOCK_CLAW,
        "Shock Claw",
        1,
        1,
        WeaponShape::Single,
        7,
        10,
        10,
        0,
        false,
        false,
    )
}

pub const fn siege_mortar() -> WeaponSpec {
    weapon(
        ids::SIEGE_MORTAR,
        "Siege Mortar",
        3,
        8,
        WeaponShape::Cross1,
        6,
        5,
        5,
        0,
        false,
        false,
    )
}

pub const fn skirmish_carbine() -> WeaponSpec {
    weapon(
        ids::SKIRMISH_CARBINE,
        "Skirmish Carbine",
        1,
        2,
        WeaponShape::Single,
        4,
        5,
        10,
        0,
        false,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flanker_matches_the_hpa637_locked_stats() {
        let flanker = flanker(UnitId(21), "Flanker", GridPos::new(0, 6));

        assert_eq!(flanker.archetype, UnitArchetype::Flanker);
        assert_eq!(flanker.faction, Faction::Enemy);
        assert_eq!(flanker.stats.max_hp, 8);
        assert_eq!(flanker.stats.armor, 0);
        assert_eq!(flanker.stats.movement, 4);
        assert_eq!(flanker.stats.accuracy, 82);
        assert_eq!(flanker.stats.evasion, 30);
        assert_eq!(flanker.hp, 8);
        assert_eq!(flanker.weapons, vec![ids::SKIRMISH_CARBINE]);
    }

    #[test]
    fn skirmish_carbine_matches_the_hpa637_locked_values() {
        let carbine = skirmish_carbine();

        assert_eq!(carbine.name, "Skirmish Carbine");
        assert_eq!((carbine.min_range, carbine.max_range), (1, 2));
        assert_eq!(carbine.shape, WeaponShape::Single);
        assert_eq!(carbine.base_damage, 4);
        assert_eq!(carbine.hit_modifier, 5);
        assert_eq!(carbine.crit_chance, 10);
        assert_eq!(carbine.en_cost, 0);
        assert!(!carbine.push);
        assert!(!carbine.counter_weapon);
    }

    #[test]
    fn shared_factories_reproduce_the_mission_one_roster() {
        let rifleman = rifleman(UnitId(11), "Rifleman L", GridPos::new(2, 3));
        let striker = striker(UnitId(13), "Striker", GridPos::new(4, 4));
        let artillery = artillery(UnitId(14), "Artillery", GridPos::new(4, 0));

        assert_eq!(rifleman.stats.max_hp, 9);
        assert_eq!(rifleman.weapons, vec![ids::SERVICE_RIFLE]);
        assert_eq!(striker.stats.max_hp, 12);
        assert_eq!(striker.weapons, vec![ids::SHOCK_CLAW]);
        assert_eq!(artillery.stats.max_hp, 10);
        assert_eq!(artillery.weapons, vec![ids::SIEGE_MORTAR]);
    }
}
