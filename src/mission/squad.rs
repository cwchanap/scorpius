//! Shared fixed Vanguard/Gunner/Interceptor roster construction from upgrade levels.
//!
//! Missions own their deployment positions and enemy openings; this module owns
//! the player squad itself so every mission builder projects the same upgrades the
//! same way. Upgrade effects per level: HP +3 max HP, Armor +1, Mobility +5
//! evasion (movement range is unchanged), Weapon +1 base damage to the mech's
//! three weapons.

use crate::campaign::model::{PlayerMech, SquadUpgrades};
use crate::domain::board::GridPos;
use crate::domain::model::{
    ActivationState, Faction, UnitArchetype, UnitId, UnitState, UnitStats, WeaponId, WeaponShape,
    WeaponSpec,
};

pub mod ids {
    use crate::domain::model::{UnitId, WeaponId};

    pub const VANGUARD: UnitId = UnitId(1);
    pub const GUNNER: UnitId = UnitId(2);
    pub const INTERCEPTOR: UnitId = UnitId(3);

    pub const PILE_LANCE: WeaponId = WeaponId(101);
    pub const REPULSOR_RAM: WeaponId = WeaponId(102);
    pub const ANCHOR_CANNON: WeaponId = WeaponId(103);
    pub const RAIL_RIFLE: WeaponId = WeaponId(104);
    pub const BURST_MISSILE: WeaponId = WeaponId(105);
    pub const OVERCHARGE_SHOT: WeaponId = WeaponId(106);
    pub const ARC_BLADE: WeaponId = WeaponId(107);
    pub const PULSE_CARBINE: WeaponId = WeaponId(108);
    pub const VECTOR_PULSE: WeaponId = WeaponId(109);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquadDeployment {
    pub vanguard: GridPos,
    pub gunner: GridPos,
    pub interceptor: GridPos,
}

/// Build the three player units and their nine weapons at the supplied
/// deployment positions, projecting each mech's upgrade levels exactly once.
pub fn build_player_squad(
    upgrades: &SquadUpgrades,
    deployment: SquadDeployment,
) -> (Vec<UnitState>, Vec<WeaponSpec>) {
    let vanguard = *upgrades.levels(PlayerMech::Vanguard);
    let gunner = *upgrades.levels(PlayerMech::Gunner);
    let interceptor = *upgrades.levels(PlayerMech::Interceptor);

    let units = vec![
        unit(
            ids::VANGUARD,
            "Vanguard",
            UnitArchetype::Vanguard,
            Faction::Player,
            stats(
                20 + 3 * vanguard.hp as i16,
                3 + vanguard.armor as i16,
                3,
                78,
                5 + 5 * vanguard.mobility as i16,
                7,
            ),
            deployment.vanguard,
            vec![ids::PILE_LANCE, ids::REPULSOR_RAM, ids::ANCHOR_CANNON],
        ),
        unit(
            ids::GUNNER,
            "Gunner",
            UnitArchetype::Gunner,
            Faction::Player,
            stats(
                12 + 3 * gunner.hp as i16,
                1 + gunner.armor as i16,
                2,
                86,
                10 + 5 * gunner.mobility as i16,
                9,
            ),
            deployment.gunner,
            vec![ids::RAIL_RIFLE, ids::BURST_MISSILE, ids::OVERCHARGE_SHOT],
        ),
        unit(
            ids::INTERCEPTOR,
            "Interceptor",
            UnitArchetype::Interceptor,
            Faction::Player,
            stats(
                15 + 3 * interceptor.hp as i16,
                1 + interceptor.armor as i16,
                4,
                82,
                20 + 5 * interceptor.mobility as i16,
                8,
            ),
            deployment.interceptor,
            vec![ids::ARC_BLADE, ids::PULSE_CARBINE, ids::VECTOR_PULSE],
        ),
    ];

    let weapons = vec![
        weapon(
            ids::PILE_LANCE,
            "Pile Lance",
            1,
            1,
            WeaponShape::Single,
            8 + vanguard.weapon as i16,
            10,
            15,
            0,
            false,
            true,
        ),
        weapon(
            ids::REPULSOR_RAM,
            "Repulsor Ram",
            1,
            1,
            WeaponShape::Single,
            5 + vanguard.weapon as i16,
            15,
            5,
            2,
            true,
            false,
        ),
        weapon(
            ids::ANCHOR_CANNON,
            "Anchor Cannon",
            2,
            3,
            WeaponShape::Single,
            6 + vanguard.weapon as i16,
            0,
            10,
            3,
            true,
            false,
        ),
        weapon(
            ids::RAIL_RIFLE,
            "Rail Rifle",
            3,
            6,
            WeaponShape::Single,
            7 + gunner.weapon as i16,
            15,
            20,
            0,
            false,
            true,
        ),
        weapon(
            ids::BURST_MISSILE,
            "Burst Missile",
            2,
            5,
            WeaponShape::Cross1,
            5 + gunner.weapon as i16,
            5,
            10,
            3,
            false,
            false,
        ),
        weapon(
            ids::OVERCHARGE_SHOT,
            "Overcharge Shot",
            2,
            6,
            WeaponShape::Single,
            10 + gunner.weapon as i16,
            -15,
            25,
            5,
            false,
            false,
        ),
        weapon(
            ids::ARC_BLADE,
            "Arc Blade",
            1,
            1,
            WeaponShape::Single,
            6 + interceptor.weapon as i16,
            15,
            15,
            0,
            false,
            false,
        ),
        weapon(
            ids::PULSE_CARBINE,
            "Pulse Carbine",
            2,
            4,
            WeaponShape::Single,
            4 + interceptor.weapon as i16,
            20,
            10,
            1,
            false,
            true,
        ),
        weapon(
            ids::VECTOR_PULSE,
            "Vector Pulse",
            1,
            2,
            WeaponShape::Single,
            4 + interceptor.weapon as i16,
            10,
            5,
            3,
            true,
            false,
        ),
    ];

    (units, weapons)
}

pub(crate) const fn stats(
    max_hp: i16,
    armor: i16,
    movement: u8,
    accuracy: i16,
    evasion: i16,
    max_en: i16,
) -> UnitStats {
    UnitStats {
        max_hp,
        armor,
        movement,
        accuracy,
        evasion,
        max_en,
    }
}

pub(crate) fn unit(
    id: UnitId,
    name: &'static str,
    archetype: UnitArchetype,
    faction: Faction,
    stats: UnitStats,
    position: GridPos,
    weapons: Vec<WeaponId>,
) -> UnitState {
    UnitState {
        id,
        name,
        archetype,
        faction,
        stats,
        hp: stats.max_hp,
        en: stats.max_en,
        position,
        weapons,
        activation: ActivationState::default(),
        reaction: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn weapon(
    id: WeaponId,
    name: &'static str,
    min_range: u8,
    max_range: u8,
    shape: WeaponShape,
    base_damage: i16,
    hit_modifier: i16,
    crit_chance: u8,
    en_cost: i16,
    push: bool,
    counter_weapon: bool,
) -> WeaponSpec {
    WeaponSpec {
        id,
        name,
        min_range,
        max_range,
        shape,
        base_damage,
        hit_modifier,
        crit_chance,
        en_cost,
        push,
        counter_weapon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::model::{SquadUpgrades, UpgradeLevels, UpgradeTrack};
    use crate::domain::board::GridPos;

    #[test]
    fn upgrades_project_once_onto_supplied_deployment() {
        let upgrades = SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 2,
                armor: 1,
                mobility: 1,
                weapon: 1,
            },
            ..Default::default()
        };
        let deployment = SquadDeployment {
            vanguard: GridPos::new(0, 0),
            gunner: GridPos::new(1, 0),
            interceptor: GridPos::new(2, 0),
        };
        let (units, weapons) = build_player_squad(&upgrades, deployment);
        let vanguard = units.iter().find(|u| u.id == ids::VANGUARD).unwrap();
        assert_eq!(vanguard.position, GridPos::new(0, 0));
        assert_eq!(vanguard.stats.max_hp, 26);
        assert_eq!(vanguard.stats.armor, 4);
        assert_eq!(vanguard.stats.evasion, 10);
        assert_eq!(vanguard.stats.movement, 3);
        assert_eq!(vanguard.hp, 26);
        assert_eq!(
            weapons
                .iter()
                .find(|w| w.id == ids::PILE_LANCE)
                .unwrap()
                .base_damage,
            9
        );
    }

    #[test]
    fn zero_upgrades_reproduce_the_hpa632_roster() {
        let deployment = SquadDeployment {
            vanguard: GridPos::new(4, 7),
            gunner: GridPos::new(3, 8),
            interceptor: GridPos::new(5, 8),
        };
        let (units, weapons) = build_player_squad(&SquadUpgrades::default(), deployment);
        assert_eq!(units.len(), 3);
        assert_eq!(weapons.len(), 9);
        let vanguard = units.iter().find(|u| u.id == ids::VANGUARD).unwrap();
        assert_eq!(vanguard.stats.max_hp, 20);
        assert_eq!(vanguard.stats.armor, 3);
        assert_eq!(vanguard.stats.evasion, 5);
        assert_eq!(
            weapons
                .iter()
                .find(|w| w.id == ids::PILE_LANCE)
                .unwrap()
                .base_damage,
            8
        );

        // Weapon upgrades project onto that mech's weapons only.
        let upgrades = SquadUpgrades {
            gunner: UpgradeLevels {
                weapon: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let (_, weapons) = build_player_squad(&upgrades, deployment);
        assert_eq!(
            weapons
                .iter()
                .find(|w| w.id == ids::RAIL_RIFLE)
                .unwrap()
                .base_damage,
            8
        );
        assert_eq!(
            weapons
                .iter()
                .find(|w| w.id == ids::PILE_LANCE)
                .unwrap()
                .base_damage,
            8
        );
    }

    #[test]
    fn every_level_accessor_feeds_the_projection() {
        let upgrades = SquadUpgrades {
            interceptor: UpgradeLevels {
                hp: 3,
                armor: 3,
                mobility: 3,
                weapon: 3,
            },
            ..Default::default()
        };
        let (units, weapons) = build_player_squad(
            &upgrades,
            SquadDeployment {
                vanguard: GridPos::new(5, 8),
                gunner: GridPos::new(4, 8),
                interceptor: GridPos::new(3, 8),
            },
        );
        let interceptor = units.iter().find(|u| u.id == ids::INTERCEPTOR).unwrap();
        assert_eq!(interceptor.stats.max_hp, 24);
        assert_eq!(interceptor.stats.armor, 4);
        assert_eq!(interceptor.stats.evasion, 35);
        assert_eq!(interceptor.en, interceptor.stats.max_en);
        assert_eq!(
            weapons
                .iter()
                .find(|w| w.id == ids::ARC_BLADE)
                .unwrap()
                .base_damage,
            9
        );
        // Un-upgraded mechs stay at base through the levels() accessor.
        assert_eq!(
            upgrades
                .levels(PlayerMech::Gunner)
                .level(UpgradeTrack::Weapon),
            0
        );
    }
}
