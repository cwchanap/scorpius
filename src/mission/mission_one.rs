use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, ExplosiveState, GridPos, Terrain},
    model::{
        EnemyOpening, MissionRules, OptionalObjective, PrimaryObjective, UnitState, WeaponSpec,
    },
};
use crate::mission::enemies;
use crate::mission::squad::{SquadDeployment, build_player_squad};
use crate::mission::{DialogueLine, DialogueScene, MissionDefinition, MissionId};

pub mod ids {
    pub use crate::mission::enemies::ids::{SERVICE_RIFLE, SHOCK_CLAW, SIEGE_MORTAR};
    pub use crate::mission::squad::ids::{
        ANCHOR_CANNON, ARC_BLADE, BURST_MISSILE, GUNNER, INTERCEPTOR, OVERCHARGE_SHOT, PILE_LANCE,
        PULSE_CARBINE, RAIL_RIFLE, REPULSOR_RAM, VANGUARD, VECTOR_PULSE,
    };

    use crate::domain::model::UnitId;

    pub const RIFLEMAN_LEFT: UnitId = UnitId(11);
    pub const RIFLEMAN_RIGHT: UnitId = UnitId(12);
    pub const STRIKER: UnitId = UnitId(13);
    pub const ARTILLERY: UnitId = UnitId(14);
}

const MISSION_ONE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

pub fn mission_one(seed: u64) -> BattleState {
    mission_one_for_campaign(seed, &SquadUpgrades::default())
}

/// Authored opening: each enemy locks its destination and intended opening
/// target before the player phase. Destinations and targets mirror the
/// original hardcoded opening exactly.
static MISSION_ONE_OPENING: [EnemyOpening; 4] = [
    EnemyOpening {
        unit: ids::RIFLEMAN_LEFT,
        destination: GridPos::new(2, 5),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::RIFLEMAN_RIGHT,
        destination: GridPos::new(6, 5),
        target: Some(ids::INTERCEPTOR),
    },
    EnemyOpening {
        unit: ids::STRIKER,
        destination: GridPos::new(4, 6),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::ARTILLERY,
        destination: GridPos::new(4, 0),
        target: Some(ids::VANGUARD),
    },
];

const MISSION_ONE_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateAllEnemies,
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_ONE_OPENING,
};

pub fn mission_one_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_ONE_DEPLOYMENT);
    units.extend(mission_one_enemy_units());
    weapons.extend(mission_one_enemy_weapons());
    BattleState::new(mission_one_board(), units, weapons, MISSION_ONE_RULES, seed)
}

fn mission_one_board() -> BoardState {
    BoardState::new(
        128,
        128,
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
    .with_terrain(mission_one_terrain)
}

/// Relay Nine occupies the northwest landing site. Roads connect the coast,
/// wooded interior and eastern mountain passes; no random terrain or islands.
fn mission_one_terrain(cell: GridPos) -> Terrain {
    let (x, y) = (i32::from(cell.x), i32::from(cell.y));
    if x < 9 && y < 9 {
        return Terrain::Plain;
    }
    let ellipse = |cx: i32, cy: i32, rx: i32, ry: i32| {
        (x - cx).pow(2) * ry.pow(2) + (y - cy).pow(2) * rx.pow(2) <= rx.pow(2) * ry.pow(2)
    };
    // A sheltered northern bay widens into the western sea.
    let coast = 13 + (y / 16 % 3) * 3 + (y % 16 - 8).abs() / 3;
    if (y >= 12 && x < coast) || (y >= 118 && x < 48 + (y - 118) * 3) {
        return Terrain::Sea;
    }
    let roads = [
        (8, 7, 34, 9),
        (32, 8, 34, 42),
        (33, 40, 76, 42),
        (74, 41, 76, 98),
        (75, 96, 119, 98),
        (117, 97, 119, 127),
        (24, 41, 26, 108),
        (25, 106, 75, 108),
        (75, 22, 112, 24),
        (74, 23, 76, 42),
    ];
    if roads.into_iter().any(|(left, top, right, bottom)| {
        (left..=right).contains(&x) && (top..=bottom).contains(&y)
    }) {
        return Terrain::Road;
    }
    if ellipse(32, 0, 14, 5)
        || ellipse(100, 18, 22, 13)
        || ellipse(104, 48, 15, 22)
        || ellipse(65, 86, 19, 9)
        || ellipse(104, 116, 11, 17)
    {
        return Terrain::Mountain;
    }
    if ellipse(16, 6, 5, 5)
        || ellipse(40, 24, 13, 12)
        || ellipse(44, 63, 18, 16)
        || ellipse(75, 57, 14, 10)
        || ellipse(46, 106, 15, 9)
        || ellipse(100, 88, 19, 15)
    {
        return Terrain::Forest;
    }
    Terrain::Plain
}

fn mission_one_enemy_units() -> Vec<UnitState> {
    vec![
        enemies::rifleman(ids::RIFLEMAN_LEFT, "Rifleman L", GridPos::new(2, 3)),
        enemies::rifleman(ids::RIFLEMAN_RIGHT, "Rifleman R", GridPos::new(6, 3)),
        enemies::striker(ids::STRIKER, "Striker", GridPos::new(4, 4)),
        enemies::artillery(ids::ARTILLERY, "Artillery", GridPos::new(4, 0)),
    ]
}

fn mission_one_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        enemies::service_rifle(),
        enemies::shock_claw(),
        enemies::siege_mortar(),
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
    unlocks: Some(MissionId::Two),
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
    use crate::domain::model::{Faction, PilotSkillState, WeaponShape};
    use crate::mission::assert_opening_plan_is_legal;

    #[test]
    fn regional_terrain_has_connected_land_and_a_legal_opening() {
        use std::collections::{BTreeSet, VecDeque};
        let mut battle = mission_one(7);
        assert_eq!(
            (battle.board().width(), battle.board().height()),
            (128, 128)
        );
        assert_eq!(GridPos::new(0, 0).manhattan(GridPos::new(127, 127)), 254);
        let board = battle.board();
        assert_eq!(board.terrain_at(GridPos::new(0, 40)), Some(Terrain::Sea));
        assert_eq!(
            board.terrain_at(GridPos::new(40, 24)),
            Some(Terrain::Forest)
        );
        assert_eq!(
            board.terrain_at(GridPos::new(100, 18)),
            Some(Terrain::Mountain)
        );
        assert_eq!(board.terrain_at(GridPos::new(75, 60)), Some(Terrain::Road));
        assert_eq!(board.terrain_at(GridPos::new(128, 127)), None);
        let start = GridPos::new(4, 7);
        let mut reached = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            for next in cell.orthogonal_neighbors(128, 128) {
                if !board.is_blocking(next) && reached.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        for y in 0..128 {
            for x in 0..128 {
                let cell = GridPos::new(x, y);
                assert!(
                    board.is_blocking(cell) || reached.contains(&cell),
                    "isolated land at {cell:?}"
                );
            }
        }
        battle.begin_round().unwrap();
        let committed = battle.intents().to_vec();
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
        assert_eq!(battle.intents(), committed);
    }

    #[test]
    fn mission_one_constructors_start_with_default_pilot_skills() {
        assert_eq!(mission_one(7).pilot_skills(), PilotSkillState::default());
        assert_eq!(
            mission_one_for_campaign(7, &SquadUpgrades::default()).pilot_skills(),
            PilotSkillState::default()
        );
    }

    #[test]
    fn mission_one_stores_its_authored_rules() {
        let battle = mission_one(7);
        let rules = battle.rules();
        assert_eq!(rules.primary, PrimaryObjective::EliminateAllEnemies);
        assert_eq!(rules.optional, OptionalObjective::Turnabout);
        assert_eq!(rules.opening_plan.len(), 4);
        assert_eq!(rules.opening_plan[0].unit, ids::RIFLEMAN_LEFT);
        assert_eq!(rules.opening_plan[0].destination, GridPos::new(2, 5));
        assert_eq!(rules.opening_plan[0].target, Some(ids::GUNNER));
    }

    #[test]
    fn mission_one_opening_rows_reference_legal_units_and_destinations() {
        let battle = mission_one(7);
        assert_opening_plan_is_legal(&battle);

        // Mission-specific: every Mission 1 opening row locks a target.
        for opening in battle.rules().opening_plan {
            assert!(opening.target.is_some(), "M1 opening rows lock a target");
        }
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
        assert_eq!(battle.board().width(), 128);
        assert_eq!(battle.board().height(), 128);
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
