use crate::domain::board::GridPos;

use super::combat::DamageSource;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectiveProgress {
    pub turnabout_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionResult {
    pub victory: bool,
    pub turnabout_complete: bool,
    pub rounds: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleEvent {
    UnitMoved {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
    AttackRolled {
        attacker: UnitId,
        weapon: WeaponId,
        target: UnitId,
        roll: u8,
        hit: bool,
        critical_roll: Option<u8>,
        critical: bool,
    },
    DamageApplied {
        target: UnitId,
        amount: i16,
        remaining_hp: i16,
        source: DamageSource,
    },
    UnitKnockedOut {
        unit: UnitId,
        position: GridPos,
    },
    UnitPushed {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
    CollisionOccurred {
        unit: UnitId,
        blocked_at: GridPos,
    },
    HazardTriggered {
        unit: UnitId,
        position: GridPos,
    },
    ExplosiveDamaged {
        position: GridPos,
        amount: i16,
        remaining_hp: i16,
        source: DamageSource,
    },
    ExplosionTriggered {
        position: GridPos,
        footprint: Vec<GridPos>,
    },
    IntentCommitted {
        attacker: UnitId,
        weapon: WeaponId,
        footprint: Vec<GridPos>,
        intended_occupant: Option<UnitId>,
    },
    IntentCanceled {
        attacker: UnitId,
    },
    AttackHitEmpty {
        attacker: UnitId,
        weapon: WeaponId,
        cell: GridPos,
    },
    CounterFired {
        defender: UnitId,
        attacker: UnitId,
        weapon: WeaponId,
    },
    OptionalObjectiveCompleted,
    MissionCompleted {
        result: MissionResult,
    },
    MissionFailed {
        result: MissionResult,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleError {
    NoUnitSelected,
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
    ActionAlreadySpent(UnitId),
    ReactionRequired(UnitId),
    EnemyResolutionNotReady,
    UnknownWeapon(WeaponId),
    WeaponNotOwned {
        unit: UnitId,
        weapon: WeaponId,
    },
    InsufficientEn {
        unit: UnitId,
        required: i16,
        available: i16,
    },
    OutOfBounds(GridPos),
    DestinationOccupied(GridPos),
    DestinationUnreachable {
        unit: UnitId,
        destination: GridPos,
    },
    InvalidTarget(GridPos),
    TargetOutOfRange {
        attacker: UnitId,
        weapon: WeaponId,
        target: GridPos,
    },
    PushTargetNotAligned {
        attacker: GridPos,
        target: GridPos,
    },
    ExplosiveNotFound(GridPos),
    NotOrthogonallyAdjacent {
        from: GridPos,
        to: GridPos,
    },
}

impl std::fmt::Display for BattleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoUnitSelected => formatter.write_str("Select a player mech first."),
            Self::UnknownUnit(unit) => write!(formatter, "Unknown unit {}.", unit.0),
            Self::WrongPhase { expected, actual } => {
                write!(
                    formatter,
                    "Command requires {expected:?}; current phase is {actual:?}."
                )
            }
            Self::ActivationInProgress(unit) => {
                write!(
                    formatter,
                    "Finish unit {} before selecting another mech.",
                    unit.0
                )
            }
            Self::UnitNotPlayer(unit) => {
                write!(formatter, "Unit {} is not player-controlled.", unit.0)
            }
            Self::UnitKnockedOut(unit) => write!(formatter, "Unit {} is knocked out.", unit.0),
            Self::ActivationAlreadyFinished(unit) => {
                write!(formatter, "Unit {} already finished this round.", unit.0)
            }
            Self::UnitNotActive(unit) => write!(formatter, "Unit {} is not active.", unit.0),
            Self::MoveAlreadySpent(unit) => write!(formatter, "Unit {} already moved.", unit.0),
            Self::ActionAlreadySpent(unit) => write!(formatter, "Unit {} already acted.", unit.0),
            Self::ReactionRequired(unit) => {
                write!(
                    formatter,
                    "Choose Counter, Guard, or Evade for unit {}.",
                    unit.0
                )
            }
            Self::EnemyResolutionNotReady => {
                formatter.write_str("Finish every surviving player mech before resolving attacks.")
            }
            Self::UnknownWeapon(weapon) => write!(formatter, "Unknown weapon {}.", weapon.0),
            Self::WeaponNotOwned { unit, weapon } => {
                write!(
                    formatter,
                    "Unit {} does not own weapon {}.",
                    unit.0, weapon.0
                )
            }
            Self::InsufficientEn {
                unit,
                required,
                available,
            } => write!(
                formatter,
                "Unit {} needs {required} EN but has {available}.",
                unit.0
            ),
            Self::OutOfBounds(position) => {
                write!(
                    formatter,
                    "Cell ({}, {}) is outside the board.",
                    position.x, position.y
                )
            }
            Self::DestinationOccupied(position) => write!(
                formatter,
                "Cell ({}, {}) is occupied.",
                position.x, position.y
            ),
            Self::DestinationUnreachable { unit, destination } => write!(
                formatter,
                "Unit {} cannot reach ({}, {}).",
                unit.0, destination.x, destination.y
            ),
            Self::InvalidTarget(position) => {
                write!(
                    formatter,
                    "Cell ({}, {}) is not a valid target.",
                    position.x, position.y
                )
            }
            Self::TargetOutOfRange {
                attacker,
                weapon,
                target,
            } => write!(
                formatter,
                "Target ({}, {}) is out of range for unit {} weapon {}.",
                target.x, target.y, attacker.0, weapon.0
            ),
            Self::PushTargetNotAligned { attacker, target } => write!(
                formatter,
                "Push requires a shared row or column: ({}, {}) to ({}, {}).",
                attacker.x, attacker.y, target.x, target.y
            ),
            Self::ExplosiveNotFound(position) => write!(
                formatter,
                "No live explosive exists at ({}, {}).",
                position.x, position.y
            ),
            Self::NotOrthogonallyAdjacent { from, to } => write!(
                formatter,
                "Cells ({}, {}) and ({}, {}) are not orthogonally adjacent.",
                from.x, from.y, to.x, to.y
            ),
        }
    }
}

impl std::error::Error for BattleError {}
