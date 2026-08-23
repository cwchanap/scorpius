use std::collections::BTreeMap;

use super::{
    board::{BoardState, GridPos},
    model::{
        ActivationState, BattleError, BattleEvent, Faction, UnitArchetype, UnitId, UnitState,
        UnitStats, WeaponId, WeaponSpec,
    },
};

#[derive(Clone, Debug)]
pub struct BattleState {
    board: BoardState,
    units: BTreeMap<UnitId, UnitState>,
    weapons: BTreeMap<WeaponId, WeaponSpec>,
}

impl BattleState {
    pub(crate) fn new(
        board: BoardState,
        units: impl IntoIterator<Item = UnitState>,
        weapons: impl IntoIterator<Item = WeaponSpec>,
    ) -> Self {
        Self {
            board,
            units: units.into_iter().map(|unit| (unit.id, unit)).collect(),
            weapons: weapons
                .into_iter()
                .map(|weapon| (weapon.id, weapon))
                .collect(),
        }
    }

    pub fn viability_fixture() -> Self {
        let stats = UnitStats {
            max_hp: 20,
            armor: 3,
            movement: 3,
            accuracy: 78,
            evasion: 5,
            max_en: 7,
        };
        Self::new(
            BoardState::empty(3, 3),
            [UnitState {
                id: UnitId(1),
                name: "Vanguard",
                archetype: UnitArchetype::Vanguard,
                faction: Faction::Player,
                stats,
                hp: stats.max_hp,
                en: stats.max_en,
                position: GridPos::new(1, 1),
                weapons: Vec::new(),
                activation: ActivationState::default(),
                reaction: None,
            }],
            [],
        )
    }

    pub const fn board(&self) -> &BoardState {
        &self.board
    }

    pub fn units(&self) -> impl Iterator<Item = &UnitState> {
        self.units.values()
    }

    pub fn unit(&self, id: UnitId) -> Option<&UnitState> {
        self.units.get(&id)
    }

    pub fn weapons(&self) -> impl Iterator<Item = &WeaponSpec> {
        self.weapons.values()
    }

    pub fn weapon(&self, id: WeaponId) -> Option<&WeaponSpec> {
        self.weapons.get(&id)
    }

    pub fn move_unit(&mut self, id: UnitId, to: GridPos) -> Result<Vec<BattleEvent>, BattleError> {
        if !self.board.contains(to) {
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
        if self
            .units
            .values()
            .any(|unit| !unit.is_knocked_out() && unit.position == to)
        {
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
