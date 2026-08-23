use std::collections::BTreeMap;

use super::{
    board::GridPos,
    model::{BattleError, BattleEvent, UnitId, UnitState},
};

pub struct BattleState {
    width: u8,
    height: u8,
    units: BTreeMap<UnitId, UnitState>,
}

impl BattleState {
    pub fn viability_fixture() -> Self {
        Self {
            width: 3,
            height: 3,
            units: [(
                UnitId(1),
                UnitState {
                    id: UnitId(1),
                    position: GridPos::new(1, 1),
                },
            )]
            .into_iter()
            .collect(),
        }
    }

    pub fn unit(&self, id: UnitId) -> Option<&UnitState> {
        self.units.get(&id)
    }

    pub fn move_unit(&mut self, id: UnitId, to: GridPos) -> Result<Vec<BattleEvent>, BattleError> {
        if to.x >= self.width || to.y >= self.height {
            return Err(BattleError::OutOfBounds(to));
        }

        let from = self
            .units
            .get(&id)
            .ok_or(BattleError::UnknownUnit(id))?
            .position;
        if from.manhattan(to) != 1 {
            return Err(BattleError::NotOrthogonallyAdjacent { from, to });
        }
        if self.units.values().any(|unit| unit.position == to) {
            return Err(BattleError::DestinationOccupied(to));
        }

        self.units
            .get_mut(&id)
            .expect("unit existence validated above")
            .position = to;

        Ok(vec![BattleEvent::UnitMoved { unit: id, from, to }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_battle_state_moves_without_bevy() {
        let mut battle = BattleState::viability_fixture();
        let events = battle
            .move_unit(UnitId(1), GridPos::new(1, 2))
            .expect("adjacent open cell is legal");

        assert_eq!(battle.unit(UnitId(1)).unwrap().position, GridPos::new(1, 2));
        assert_eq!(
            events,
            vec![BattleEvent::UnitMoved {
                unit: UnitId(1),
                from: GridPos::new(1, 1),
                to: GridPos::new(1, 2),
            }]
        );
    }
}
