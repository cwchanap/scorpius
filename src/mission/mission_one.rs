use crate::domain::{
    battle::BattleState,
    board::{BoardState, ExplosiveState, GridPos},
    model::{
        ActivationState, Faction, UnitArchetype, UnitId, UnitState, UnitStats, WeaponId,
        WeaponShape, WeaponSpec,
    },
};

pub mod ids {
    use crate::domain::model::{UnitId, WeaponId};

    pub const VANGUARD: UnitId = UnitId(1);
    pub const GUNNER: UnitId = UnitId(2);
    pub const INTERCEPTOR: UnitId = UnitId(3);
    pub const RIFLEMAN_LEFT: UnitId = UnitId(11);
    pub const RIFLEMAN_RIGHT: UnitId = UnitId(12);
    pub const STRIKER: UnitId = UnitId(13);
    pub const ARTILLERY: UnitId = UnitId(14);

    pub const PILE_LANCE: WeaponId = WeaponId(101);
    pub const REPULSOR_RAM: WeaponId = WeaponId(102);
    pub const ANCHOR_CANNON: WeaponId = WeaponId(103);
    pub const RAIL_RIFLE: WeaponId = WeaponId(104);
    pub const BURST_MISSILE: WeaponId = WeaponId(105);
    pub const OVERCHARGE_SHOT: WeaponId = WeaponId(106);
    pub const ARC_BLADE: WeaponId = WeaponId(107);
    pub const PULSE_CARBINE: WeaponId = WeaponId(108);
    pub const VECTOR_PULSE: WeaponId = WeaponId(109);
    pub const SERVICE_RIFLE: WeaponId = WeaponId(201);
    pub const SHOCK_CLAW: WeaponId = WeaponId(202);
    pub const SIEGE_MORTAR: WeaponId = WeaponId(203);
}

pub fn mission_one(seed: u64) -> BattleState {
    let board = BoardState::new(
        9,
        9,
        [
            GridPos::new(2, 1),
            GridPos::new(6, 1),
            GridPos::new(1, 4),
            GridPos::new(7, 4),
            GridPos::new(3, 5),
            GridPos::new(5, 5),
        ],
        [GridPos::new(2, 6)],
        [ExplosiveState {
            position: GridPos::new(6, 6),
            hp: 4,
            exploded: false,
        }],
    );

    let units = [
        unit(
            ids::VANGUARD,
            "Vanguard",
            UnitArchetype::Vanguard,
            Faction::Player,
            stats(20, 3, 3, 78, 5, 7),
            GridPos::new(4, 7),
            vec![ids::PILE_LANCE, ids::REPULSOR_RAM, ids::ANCHOR_CANNON],
        ),
        unit(
            ids::GUNNER,
            "Gunner",
            UnitArchetype::Gunner,
            Faction::Player,
            stats(12, 1, 2, 86, 10, 9),
            GridPos::new(3, 8),
            vec![ids::RAIL_RIFLE, ids::BURST_MISSILE, ids::OVERCHARGE_SHOT],
        ),
        unit(
            ids::INTERCEPTOR,
            "Interceptor",
            UnitArchetype::Interceptor,
            Faction::Player,
            stats(15, 1, 4, 82, 20, 8),
            GridPos::new(5, 8),
            vec![ids::ARC_BLADE, ids::PULSE_CARBINE, ids::VECTOR_PULSE],
        ),
        unit(
            ids::RIFLEMAN_LEFT,
            "Rifleman L",
            UnitArchetype::Rifleman,
            Faction::Enemy,
            stats(9, 1, 2, 72, 5, 0),
            GridPos::new(2, 3),
            vec![ids::SERVICE_RIFLE],
        ),
        unit(
            ids::RIFLEMAN_RIGHT,
            "Rifleman R",
            UnitArchetype::Rifleman,
            Faction::Enemy,
            stats(9, 1, 2, 72, 5, 0),
            GridPos::new(6, 3),
            vec![ids::SERVICE_RIFLE],
        ),
        unit(
            ids::STRIKER,
            "Striker",
            UnitArchetype::Striker,
            Faction::Enemy,
            stats(12, 2, 2, 78, 10, 0),
            GridPos::new(4, 4),
            vec![ids::SHOCK_CLAW],
        ),
        unit(
            ids::ARTILLERY,
            "Artillery",
            UnitArchetype::Artillery,
            Faction::Enemy,
            stats(10, 1, 1, 90, 0, 0),
            GridPos::new(4, 0),
            vec![ids::SIEGE_MORTAR],
        ),
    ];

    let weapons = [
        weapon(
            ids::PILE_LANCE,
            "Pile Lance",
            1,
            1,
            WeaponShape::Single,
            8,
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
            5,
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
            6,
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
            7,
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
            5,
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
            10,
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
            6,
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
            4,
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
            4,
            10,
            5,
            3,
            true,
            false,
        ),
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
        ),
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
        ),
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
        ),
    ];

    BattleState::new(board, units, weapons, seed)
}

const fn stats(
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

fn unit(
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
const fn weapon(
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

    #[test]
    fn mission_one_has_the_locked_roster_and_nine_player_weapons() {
        let battle = mission_one(7);
        let players: Vec<_> = battle
            .units()
            .filter(|unit| unit.faction == Faction::Player)
            .collect();
        let enemies: Vec<_> = battle
            .units()
            .filter(|unit| unit.faction == Faction::Enemy)
            .collect();

        assert_eq!(players.len(), 3);
        assert_eq!(enemies.len(), 4);
        assert_eq!(
            players.iter().map(|unit| unit.weapons.len()).sum::<usize>(),
            9
        );
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert!(battle.board().is_blocking(GridPos::new(3, 5)));
        assert!(battle.board().is_hazard(GridPos::new(2, 6)));
        assert_eq!(
            battle.board().explosive_at(GridPos::new(6, 6)).unwrap().hp,
            4
        );
    }

    #[test]
    fn weapon_values_match_the_approved_design() {
        let battle = mission_one(7);
        let expected = [
            (
                ids::PILE_LANCE,
                "Pile Lance",
                1,
                1,
                WeaponShape::Single,
                8,
                10,
                15,
                0,
                false,
                true,
            ),
            (
                ids::REPULSOR_RAM,
                "Repulsor Ram",
                1,
                1,
                WeaponShape::Single,
                5,
                15,
                5,
                2,
                true,
                false,
            ),
            (
                ids::ANCHOR_CANNON,
                "Anchor Cannon",
                2,
                3,
                WeaponShape::Single,
                6,
                0,
                10,
                3,
                true,
                false,
            ),
            (
                ids::RAIL_RIFLE,
                "Rail Rifle",
                3,
                6,
                WeaponShape::Single,
                7,
                15,
                20,
                0,
                false,
                true,
            ),
            (
                ids::BURST_MISSILE,
                "Burst Missile",
                2,
                5,
                WeaponShape::Cross1,
                5,
                5,
                10,
                3,
                false,
                false,
            ),
            (
                ids::OVERCHARGE_SHOT,
                "Overcharge Shot",
                2,
                6,
                WeaponShape::Single,
                10,
                -15,
                25,
                5,
                false,
                false,
            ),
            (
                ids::ARC_BLADE,
                "Arc Blade",
                1,
                1,
                WeaponShape::Single,
                6,
                15,
                15,
                0,
                false,
                false,
            ),
            (
                ids::PULSE_CARBINE,
                "Pulse Carbine",
                2,
                4,
                WeaponShape::Single,
                4,
                20,
                10,
                1,
                false,
                true,
            ),
            (
                ids::VECTOR_PULSE,
                "Vector Pulse",
                1,
                2,
                WeaponShape::Single,
                4,
                10,
                5,
                3,
                true,
                false,
            ),
            (
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
            ),
            (
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
            ),
            (
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
            ),
        ];

        for (id, name, min, max, shape, damage, hit, crit, en, push, counter) in expected {
            let weapon = battle.weapon(id).unwrap();
            assert_eq!(weapon.name, name);
            assert_eq!((weapon.min_range, weapon.max_range), (min, max));
            assert_eq!(weapon.shape, shape);
            assert_eq!(weapon.base_damage, damage);
            assert_eq!(weapon.hit_modifier, hit);
            assert_eq!(weapon.crit_chance, crit);
            assert_eq!(weapon.en_cost, en);
            assert_eq!(weapon.push, push);
            assert_eq!(weapon.counter_weapon, counter);
        }
    }

    #[test]
    fn board_layout_matches_the_approved_coordinates() {
        let battle = mission_one(7);
        assert_eq!(
            battle.board().blocking_cells().collect::<Vec<_>>(),
            vec![
                GridPos::new(2, 1),
                GridPos::new(6, 1),
                GridPos::new(1, 4),
                GridPos::new(7, 4),
                GridPos::new(3, 5),
                GridPos::new(5, 5),
            ]
        );
        assert_eq!(
            battle.board().hazard_cells().collect::<Vec<_>>(),
            vec![GridPos::new(2, 6)]
        );
    }
}
