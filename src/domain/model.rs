use crate::domain::board::GridPos;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WeaponId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Faction {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitArchetype {
    Vanguard,
    Gunner,
    Interceptor,
    Rifleman,
    Striker,
    Artillery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitStats {
    pub max_hp: i16,
    pub armor: i16,
    pub movement: u8,
    pub accuracy: i16,
    pub evasion: i16,
    pub max_en: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponShape {
    Single,
    Cross1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeaponSpec {
    pub id: WeaponId,
    pub name: &'static str,
    pub min_range: u8,
    pub max_range: u8,
    pub shape: WeaponShape,
    pub base_damage: i16,
    pub hit_modifier: i16,
    pub crit_chance: u8,
    pub en_cost: i16,
    pub push: bool,
    pub counter_weapon: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reaction {
    Counter,
    Guard,
    Evade,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActivationState {
    pub moved: bool,
    pub acted: bool,
    pub finished: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitState {
    pub id: UnitId,
    pub name: &'static str,
    pub archetype: UnitArchetype,
    pub faction: Faction,
    pub stats: UnitStats,
    pub hp: i16,
    pub en: i16,
    pub position: GridPos,
    pub weapons: Vec<WeaponId>,
    pub activation: ActivationState,
    pub reaction: Option<Reaction>,
}

impl UnitState {
    pub fn is_knocked_out(&self) -> bool {
        self.hp == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattlePhase {
    EnemyPlanning,
    Player,
    EnemyResolution,
    Victory,
    Defeat,
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
    WrongPhase {
        expected: BattlePhase,
        actual: BattlePhase,
    },
    ActivationInProgress(UnitId),
    UnitNotPlayer(UnitId),
    UnitKnockedOut(UnitId),
    ActivationAlreadyFinished(UnitId),
    UnitNotActive(UnitId),
    MoveAlreadySpent(UnitId),
    ReactionRequired(UnitId),
    OutOfBounds(GridPos),
    DestinationOccupied(GridPos),
    DestinationUnreachable {
        unit: UnitId,
        destination: GridPos,
    },
    NotOrthogonallyAdjacent {
        from: GridPos,
        to: GridPos,
    },
}
