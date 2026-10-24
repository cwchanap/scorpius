//! Regional battlefields for Missions 2–7.
//!
//! Each mission keeps its authored 9×9 arena in the northwest corner of a
//! 64×64 region. A wall on the arena's east and south rim keeps the authored
//! push edges: a shove off the old board edge still collides for 3 damage.
//! One gate at (9, 8) opens onto the east road, which is where each mission's
//! reinforcement wave arrives. Because the gate is the only way out, no path
//! between two arena cells ever gets shorter by leaving the arena.

use crate::domain::board::{BoardState, ExplosiveState, GridPos, Terrain};

/// Side length of every Mission 2–7 region.
pub const REGION_SIZE: u8 = 64;
/// Side length of the authored arena in the region's northwest corner.
pub const ARENA_SIZE: u8 = 9;
/// The one opening in the arena wall, on the east road.
pub const ARENA_GATE: GridPos = GridPos::new(ARENA_SIZE, 8);

/// Inclusive `(left, top, right, bottom)` cells.
#[derive(Clone, Copy, Debug)]
pub struct Strip(pub i32, pub i32, pub i32, pub i32);

/// An ellipse `(center x, center y, radius x, radius y)` in cells.
#[derive(Clone, Copy, Debug)]
pub struct Oval(pub i32, pub i32, pub i32, pub i32);

impl Strip {
    fn contains(self, x: i32, y: i32) -> bool {
        (self.0..=self.2).contains(&x) && (self.1..=self.3).contains(&y)
    }
}

impl Oval {
    fn contains(self, x: i32, y: i32) -> bool {
        let Self(cx, cy, rx, ry) = self;
        (x - cx).pow(2) * ry.pow(2) + (y - cy).pow(2) * rx.pow(2) <= rx.pow(2) * ry.pow(2)
    }
}

/// Authored terrain for one region. Roads win over water (they bridge it),
/// then water, mountains and forests; everything else is plains. The arena
/// itself is always plains, so the authored fight is unchanged.
#[derive(Debug)]
pub struct Region {
    pub roads: &'static [Strip],
    pub water: &'static [Oval],
    pub mountains: &'static [Oval],
    pub forests: &'static [Oval],
}

impl Region {
    pub fn terrain(&self, cell: GridPos) -> Terrain {
        if cell.x < ARENA_SIZE && cell.y < ARENA_SIZE {
            return Terrain::Plain;
        }
        let (x, y) = (i32::from(cell.x), i32::from(cell.y));
        if self.roads.iter().any(|road| road.contains(x, y)) {
            Terrain::Road
        } else if self.water.iter().any(|water| water.contains(x, y)) {
            Terrain::Sea
        } else if self.mountains.iter().any(|peak| peak.contains(x, y)) {
            Terrain::Mountain
        } else if self.forests.iter().any(|wood| wood.contains(x, y)) {
            Terrain::Forest
        } else {
            Terrain::Plain
        }
    }
}

/// The arena's east and south rim, minus the gate.
pub fn arena_wall() -> impl Iterator<Item = GridPos> {
    (0..ARENA_SIZE)
        .map(|y| GridPos::new(ARENA_SIZE, y))
        .filter(|cell| *cell != ARENA_GATE)
        .chain((0..ARENA_SIZE).map(|x| GridPos::new(x, ARENA_SIZE)))
}

/// Builds a region around an authored arena's obstacles, hazards and props.
pub fn regional_board(
    blocking: impl IntoIterator<Item = GridPos>,
    hazards: impl IntoIterator<Item = GridPos>,
    explosives: impl IntoIterator<Item = ExplosiveState>,
    region: &'static Region,
) -> BoardState {
    BoardState::new(
        REGION_SIZE,
        REGION_SIZE,
        blocking.into_iter().chain(arena_wall()),
        hazards,
        explosives,
    )
    .with_terrain(|cell| region.terrain(cell))
}

/// The authored arena's obstacles, without the arena wall.
#[cfg(test)]
pub(crate) fn arena_blocking(battle: &crate::domain::battle::BattleState) -> Vec<GridPos> {
    battle
        .board()
        .blocking_cells()
        .filter(|cell| cell.x < ARENA_SIZE && cell.y < ARENA_SIZE)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Faction;
    use crate::mission::{MissionId, mission_definition};
    use crate::{campaign::model::SquadUpgrades, domain::battle::BattleState};
    use std::collections::{BTreeSet, VecDeque};

    const REGIONAL: [MissionId; 6] = [
        MissionId::Two,
        MissionId::Three,
        MissionId::Four,
        MissionId::Five,
        MissionId::Six,
        MissionId::Seven,
    ];

    fn build(id: MissionId) -> BattleState {
        (mission_definition(id).unwrap().build)(7, &SquadUpgrades::default())
    }

    fn reachable(battle: &BattleState, start: GridPos) -> BTreeSet<GridPos> {
        let board = battle.board();
        let mut reached = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            for next in cell.orthogonal_neighbors(board.width(), board.height()) {
                if !board.is_blocking(next) && reached.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        reached
    }

    #[test]
    fn arena_wall_closes_the_old_edges_except_the_gate() {
        let wall: BTreeSet<_> = arena_wall().collect();
        assert_eq!(wall.len(), 17);
        assert!(!wall.contains(&ARENA_GATE));
        assert!(wall.contains(&GridPos::new(9, 0)));
        assert!(wall.contains(&GridPos::new(9, 7)));
        assert!(wall.contains(&GridPos::new(0, 9)));
        assert!(wall.contains(&GridPos::new(8, 9)));
    }

    #[test]
    fn every_region_keeps_its_arena_and_connects_all_its_land() {
        for id in REGIONAL {
            let battle = build(id);
            let board = battle.board();
            assert_eq!((board.width(), board.height()), (64, 64), "{id}");
            for cell in arena_wall() {
                assert!(board.is_blocking(cell), "{id}: wall open at {cell:?}");
            }
            assert!(!board.is_blocking(ARENA_GATE), "{id}: gate closed");
            for y in 0..ARENA_SIZE {
                for x in 0..ARENA_SIZE {
                    let cell = GridPos::new(x, y);
                    assert_eq!(board.terrain_at(cell), Some(Terrain::Plain), "{id}");
                }
            }
            let squad = battle
                .units()
                .find(|unit| unit.faction == Faction::Player)
                .unwrap();
            let reached = reachable(&battle, squad.position);
            for y in 0..REGION_SIZE {
                for x in 0..REGION_SIZE {
                    let cell = GridPos::new(x, y);
                    assert!(
                        board.is_blocking(cell) || reached.contains(&cell),
                        "{id}: isolated land at {cell:?}"
                    );
                }
            }
            let kinds: BTreeSet<_> = (0..REGION_SIZE)
                .flat_map(|y| (0..REGION_SIZE).map(move |x| GridPos::new(x, y)))
                .filter_map(|cell| board.terrain_at(cell))
                .map(|terrain| format!("{terrain:?}"))
                .collect();
            assert_eq!(kinds.len(), 5, "{id}: every terrain kind appears");
        }
    }

    #[test]
    fn clearing_the_arena_calls_the_wave_in_instead_of_winning() {
        use crate::domain::combat::DamageSource;
        use crate::mission::squad::ids::RAIL_RIFLE;
        for id in [MissionId::Two, MissionId::Five] {
            let mut battle = build(id);
            battle.begin_round().unwrap();
            let deployed: Vec<_> = battle
                .units()
                .filter(|unit| unit.faction == Faction::Enemy)
                .map(|unit| unit.id)
                .collect();
            for enemy in deployed {
                battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
            }
            assert!(battle.result().is_none(), "{id}: the wave is still inbound");
            assert!(battle.pending_reinforcements().count() > 0, "{id}");
        }
    }

    #[test]
    fn every_region_lands_its_wave_on_the_road_outside_the_arena() {
        for id in REGIONAL {
            let battle = build(id);
            let wave: Vec<_> = battle.pending_reinforcements().collect();
            assert!(!wave.is_empty(), "{id}: has a reinforcement wave");
            for unit in wave {
                let cell = unit.position;
                assert!(cell.x > ARENA_SIZE, "{id}: {} arrives outside", unit.name);
                assert_eq!(battle.board().terrain_at(cell), Some(Terrain::Road), "{id}");
                assert!(battle.occupant_at(cell).is_none(), "{id}");
            }
        }
    }
}
