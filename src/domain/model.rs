use crate::domain::board::GridPos;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitState {
    pub id: UnitId,
    pub position: GridPos,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleEvent {
    UnitMoved {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleError {
    UnknownUnit(UnitId),
    OutOfBounds(GridPos),
    DestinationOccupied(GridPos),
    NotOrthogonallyAdjacent { from: GridPos, to: GridPos },
}
