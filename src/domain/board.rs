use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridPos {
    pub x: u8,
    pub y: u8,
}

impl GridPos {
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    pub fn manhattan(self, other: Self) -> u8 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }

    pub fn orthogonal_neighbors(self, width: u8, height: u8) -> Vec<Self> {
        let mut neighbors = Vec::with_capacity(4);
        if self.y > 0 {
            neighbors.push(Self::new(self.x, self.y - 1));
        }
        if self.x > 0 {
            neighbors.push(Self::new(self.x - 1, self.y));
        }
        if self.x + 1 < width {
            neighbors.push(Self::new(self.x + 1, self.y));
        }
        if self.y + 1 < height {
            neighbors.push(Self::new(self.x, self.y + 1));
        }
        neighbors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplosiveState {
    pub position: GridPos,
    pub hp: i16,
    pub exploded: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Terrain {
    #[default]
    Plain,
    Road,
    Forest,
    Sea,
    Mountain,
}

impl Terrain {
    pub const fn movement_cost(self) -> Option<u8> {
        match self {
            Self::Plain | Self::Road => Some(1),
            Self::Forest => Some(2),
            Self::Sea | Self::Mountain => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Plain => "Plains",
            Self::Road => "Road",
            Self::Forest => "Forest",
            Self::Sea => "Sea",
            Self::Mountain => "Mountain",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardState {
    width: u8,
    height: u8,
    terrain: Vec<Terrain>,
    blocking: BTreeSet<GridPos>,
    hazards: BTreeSet<GridPos>,
    explosives: BTreeMap<GridPos, ExplosiveState>,
}

impl BoardState {
    pub(crate) fn new(
        width: u8,
        height: u8,
        blocking: impl IntoIterator<Item = GridPos>,
        hazards: impl IntoIterator<Item = GridPos>,
        explosives: impl IntoIterator<Item = ExplosiveState>,
    ) -> Self {
        Self {
            width,
            height,
            terrain: vec![Terrain::Plain; usize::from(width) * usize::from(height)],
            blocking: blocking.into_iter().collect(),
            hazards: hazards.into_iter().collect(),
            explosives: explosives
                .into_iter()
                .map(|explosive| (explosive.position, explosive))
                .collect(),
        }
    }

    pub(crate) fn empty(width: u8, height: u8) -> Self {
        Self::new(width, height, [], [], [])
    }

    pub(crate) fn with_terrain(mut self, terrain: impl Fn(GridPos) -> Terrain) -> Self {
        for y in 0..self.height {
            for x in 0..self.width {
                self.terrain[usize::from(y) * usize::from(self.width) + usize::from(x)] =
                    terrain(GridPos::new(x, y));
            }
        }
        self
    }

    pub fn terrain_at(&self, position: GridPos) -> Option<Terrain> {
        self.contains(position).then(|| {
            self.terrain
                [usize::from(position.y) * usize::from(self.width) + usize::from(position.x)]
        })
    }

    pub fn movement_cost(&self, position: GridPos) -> Option<u8> {
        if self.blocking.contains(&position) {
            return None;
        }
        self.terrain_at(position)?.movement_cost()
    }

    pub const fn width(&self) -> u8 {
        self.width
    }

    pub const fn height(&self) -> u8 {
        self.height
    }

    pub fn contains(&self, position: GridPos) -> bool {
        position.x < self.width && position.y < self.height
    }

    pub fn is_blocking(&self, position: GridPos) -> bool {
        self.movement_cost(position).is_none()
    }

    pub fn is_hazard(&self, position: GridPos) -> bool {
        self.hazards.contains(&position)
    }

    pub fn explosive_at(&self, position: GridPos) -> Option<&ExplosiveState> {
        self.explosives.get(&position)
    }

    pub(crate) fn explosive_at_mut(&mut self, position: GridPos) -> Option<&mut ExplosiveState> {
        self.explosives.get_mut(&position)
    }

    pub fn has_live_explosive(&self, position: GridPos) -> bool {
        self.explosive_at(position)
            .is_some_and(|explosive| explosive.hp > 0 && !explosive.exploded)
    }

    /// Structural obstacles; natural barriers are exposed through `terrain_at`.
    pub fn blocking_cells(&self) -> impl Iterator<Item = GridPos> + '_ {
        let mut cells: Vec<_> = self.blocking.iter().copied().collect();
        cells.sort_by_key(|position| (position.y, position.x));
        cells.into_iter()
    }

    pub fn hazard_cells(&self) -> impl Iterator<Item = GridPos> + '_ {
        let mut cells: Vec<_> = self.hazards.iter().copied().collect();
        cells.sort_by_key(|position| (position.y, position.x));
        cells.into_iter()
    }

    pub fn explosives(&self) -> impl Iterator<Item = &ExplosiveState> {
        self.explosives.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_distance_and_neighbors_are_orthogonal() {
        let origin = GridPos::new(2, 3);
        assert_eq!(origin.manhattan(GridPos::new(5, 1)), 5);
        assert_eq!(
            origin.orthogonal_neighbors(5, 5),
            vec![
                GridPos::new(2, 2),
                GridPos::new(1, 3),
                GridPos::new(3, 3),
                GridPos::new(2, 4),
            ]
        );
    }
}
