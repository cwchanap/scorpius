use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, ExplosiveState, GridPos},
    model::{Faction, UnitArchetype, UnitState, WeaponShape, WeaponSpec},
};
use crate::mission::squad::{SquadDeployment, build_player_squad, stats, unit, weapon};
use crate::mission::{DialogueLine, DialogueScene, MissionDefinition, MissionId};

pub mod ids {
    pub use crate::mission::squad::ids::{
        ANCHOR_CANNON, ARC_BLADE, BURST_MISSILE, GUNNER, INTERCEPTOR, OVERCHARGE_SHOT, PILE_LANCE,
        PULSE_CARBINE, RAIL_RIFLE, REPULSOR_RAM, VANGUARD, VECTOR_PULSE,
    };

    use crate::domain::model::{UnitId, WeaponId};

    pub const RIFLEMAN_LEFT: UnitId = UnitId(11);
    pub const RIFLEMAN_RIGHT: UnitId = UnitId(12);
    pub const STRIKER: UnitId = UnitId(13);
    pub const ARTILLERY: UnitId = UnitId(14);

    pub const SERVICE_RIFLE: WeaponId = WeaponId(201);
    pub const SHOCK_CLAW: WeaponId = WeaponId(202);
    pub const SIEGE_MORTAR: WeaponId = WeaponId(203);
}

const MISSION_ONE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

pub fn mission_one(seed: u64) -> BattleState {
    mission_one_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_one_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_ONE_DEPLOYMENT);
    units.extend(mission_one_enemy_units());
    weapons.extend(mission_one_enemy_weapons());
    BattleState::new(mission_one_board(), units, weapons, seed)
}

fn mission_one_board() -> BoardState {
    BoardState::new(
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
    )
}

fn mission_one_enemy_units() -> Vec<UnitState> {
    vec![
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
    ]
}

fn mission_one_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
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
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "Squad, Relay Nine is broadcasting an enemy garrison signal. Four hostiles hold the relay.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Understood. We punch through, clear the board, and take the relay back.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Warning: their artillery is already locking onto you. Make their own firepower work against you.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Relay Nine is ours. The board is clear and the squad is intact.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Confirmed. Salvage recovered — spend it before the next drop.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_ONE_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::One,
    unlocks: MissionId::Two,
    build: mission_one_for_campaign,
    title: "Mission 1 — Turnabout at Relay Nine",
    primary_objective: "Eliminate all enemies.",
    optional_objective: "Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.",
    base_reward: 300,
    optional_reward: 100,
    pre_mission: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &PRE_MISSION_LINES,
    },
    aftermath: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &AFTERMATH_LINES,
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::model::UpgradeLevels;
    use crate::domain::model::PilotSkillState;

    #[test]
    fn mission_one_constructors_start_with_default_pilot_skills() {
        assert_eq!(mission_one(7).pilot_skills(), PilotSkillState::default());
        assert_eq!(
            mission_one_for_campaign(7, &SquadUpgrades::default()).pilot_skills(),
            PilotSkillState::default()
        );
    }

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
    fn upgraded_construction_keeps_the_mission_one_deployment() {
        let upgrades = SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 2,
                armor: 1,
                mobility: 1,
                weapon: 1,
            },
            ..Default::default()
        };
        let battle = mission_one_for_campaign(7, &upgrades);
        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().position,
            GridPos::new(4, 7)
        );
        assert_eq!(
            battle.unit(ids::GUNNER).unwrap().position,
            GridPos::new(3, 8)
        );
        assert_eq!(
            battle.unit(ids::INTERCEPTOR).unwrap().position,
            GridPos::new(5, 8)
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
