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
