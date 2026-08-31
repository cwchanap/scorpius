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
    pub const BASTION_CANNON: WeaponId = WeaponId(205);
    pub const IMPULSE_PROJECTOR: WeaponId = WeaponId(206);
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

pub fn bulwark(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Bulwark,
        Faction::Enemy,
        stats(16, 4, 1, 76, 0, 0),
        position,
        vec![ids::BASTION_CANNON],
    )
}

pub fn controller(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(
        id,
        name,
        UnitArchetype::Controller,
        Faction::Enemy,
        stats(9, 1, 2, 82, 15, 0),
        position,
        vec![ids::IMPULSE_PROJECTOR],
    )
}

pub const fn bastion_cannon() -> WeaponSpec {
    weapon(
        ids::BASTION_CANNON,
        "Bastion Cannon",
        1,
        3,
        WeaponShape::Single,
        6,
        0,
        5,
        0,
        false,
        false,
    )
}

pub const fn impulse_projector() -> WeaponSpec {
    weapon(
        ids::IMPULSE_PROJECTOR,
        "Impulse Projector",
        2,
        4,
        WeaponShape::Single,
        3,
        10,
        0,
        0,
        true,
        false,
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
    fn bulwark_matches_the_hpa523_locked_stats() {
        let bulwark = bulwark(UnitId(41), "Gate Bulwark", GridPos::new(4, 5));

        assert_eq!(bulwark.archetype, UnitArchetype::Bulwark);
        assert_eq!(bulwark.faction, Faction::Enemy);
        assert_eq!(bulwark.stats.max_hp, 16);
        assert_eq!(bulwark.stats.armor, 4);
        assert_eq!(bulwark.stats.movement, 1);
        assert_eq!(bulwark.stats.accuracy, 76);
        assert_eq!(bulwark.stats.evasion, 0);
        assert_eq!(bulwark.hp, 16);
        assert_eq!(bulwark.weapons, vec![ids::BASTION_CANNON]);
    }

    #[test]
    fn controller_matches_the_hpa523_locked_stats() {
        let controller = controller(UnitId(42), "Controller", GridPos::new(0, 7));

        assert_eq!(controller.archetype, UnitArchetype::Controller);
        assert_eq!(controller.faction, Faction::Enemy);
        assert_eq!(controller.stats.max_hp, 9);
        assert_eq!(controller.stats.armor, 1);
        assert_eq!(controller.stats.movement, 2);
        assert_eq!(controller.stats.accuracy, 82);
        assert_eq!(controller.stats.evasion, 15);
        assert_eq!(controller.hp, 9);
        assert_eq!(controller.weapons, vec![ids::IMPULSE_PROJECTOR]);
    }

    #[test]
    fn impulse_projector_matches_the_hpa523_locked_values() {
        let projector = impulse_projector();

        assert_eq!(projector.id, ids::IMPULSE_PROJECTOR);
        assert_eq!(projector.name, "Impulse Projector");
        assert_eq!((projector.min_range, projector.max_range), (2, 4));
        assert_eq!(projector.shape, WeaponShape::Single);
        assert_eq!(projector.base_damage, 3);
        assert_eq!(projector.hit_modifier, 10);
        assert_eq!(projector.crit_chance, 0);
        assert_eq!(projector.en_cost, 0);
        assert!(projector.push);
        assert!(!projector.counter_weapon);
    }

    #[test]
    fn bastion_cannon_matches_the_hpa523_locked_values() {
        let cannon = bastion_cannon();

        assert_eq!(cannon.id, ids::BASTION_CANNON);
        assert_eq!(cannon.name, "Bastion Cannon");
        assert_eq!((cannon.min_range, cannon.max_range), (1, 3));
        assert_eq!(cannon.shape, WeaponShape::Single);
        assert_eq!(cannon.base_damage, 6);
        assert_eq!(cannon.hit_modifier, 0);
        assert_eq!(cannon.crit_chance, 5);
        assert_eq!(cannon.en_cost, 0);
        assert!(!cannon.push);
        assert!(!cannon.counter_weapon);
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
