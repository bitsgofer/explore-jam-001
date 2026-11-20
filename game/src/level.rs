use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, Index, IndexMut, Neg, Sub};

use bevy::prelude::{
    App, AppExtStates, Commands, Component, DespawnOnExit, FromWorld, Name, OnEnter, Plugin, Res,
    Resource, State, States, Transform, Vec2, Visibility, World,
};
use bevy::sprite::Anchor;

use crate::global;
use crate::spritesheet::SpriteSheet;
use crate::spritesheet::SpriteSheets;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Levels>()
            .insert_state(Level::_2)
            .add_systems(OnEnter(Level::_0), spawn_level)
            .add_systems(OnEnter(Level::_1), spawn_level)
            .add_systems(OnEnter(Level::_2), spawn_level);
    }
}

fn spawn_level(
    mut commands: Commands,
    sheets: Res<SpriteSheets>,
    levels: Res<Levels>,
    current_level: Res<State<Level>>,
) {
    let lvl = *current_level.get();
    let grid = levels.get(lvl);
    let sheet = sheets.get(SpriteSheet::Level);
    commands
        .spawn((LevelRoot, DespawnOnExit(lvl)))
        .with_children(|level| {
            for (_, pos, tile) in grid.iter() {
                level.spawn((
                    tile,
                    pos,
                    sheet.sprite(tile.index_in_tile_spritesheet()),
                    Transform::from_translation(pos.to_xy().extend(global::LEVEL_Z)),
                    Anchor::BOTTOM_LEFT,
                    Name::new(format!("Tile({}, {})", pos.row, pos.col)),
                ));
            }
        });
    return;
}

/// The `(row, col)` position of a Tile in the TileGrid.
#[derive(Clone, Copy, Debug, Component, Eq, PartialEq, Default)]
pub struct GridVec2 {
    pub row: i32,
    pub col: i32,
}

impl GridVec2 {
    pub const ZERO: Self = Self { row: 0, col: 0 };
    pub const DOWN: Self = Self { row: -1, col: 0 };
    pub const LEFT: Self = Self { row: 0, col: -1 };
    pub const UP: Self = Self { row: 1, col: 0 };
    pub const RIGHT: Self = Self { row: 0, col: 1 };

    pub fn new(row: i32, col: i32) -> Self {
        return GridVec2 { row, col };
    }

    /// Return `(x, y)` (world) coordinates of the cell's anchor
    /// (its bottom-left corner).
    pub fn to_xy(self) -> Vec2 {
        return Vec2::new(
            self.col as f32 * global::TILE_SIZE_PIXELS as f32,
            self.row as f32 * global::TILE_SIZE_PIXELS as f32,
        );
    }

    /// Translate the original GridPos to a new coordinate.
    pub fn translate(self, delta_row: i32, delta_col: i32) -> GridVec2 {
        GridVec2 {
            row: self.row + delta_row,
            col: self.col + delta_col,
        }
    }

    pub fn abs(self) -> GridVec2 {
        let r = if self.row < 0 { -self.row } else { self.row };
        let c = if self.col < 0 { -self.col } else { self.col };
        GridVec2 { row: r, col: c }
    }
}

impl fmt::Display for GridVec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(r{}, c{})", self.row, self.col)
    }
}

impl Add<GridVec2> for GridVec2 {
    type Output = GridVec2;
    fn add(self, d: GridVec2) -> GridVec2 {
        return GridVec2 {
            row: self.row + d.row,
            col: self.col + d.col,
        };
    }
}

impl Neg for GridVec2 {
    type Output = GridVec2;
    fn neg(self) -> GridVec2 {
        return GridVec2 {
            row: -self.row,
            col: -self.col,
        };
    }
}

impl Sub<GridVec2> for GridVec2 {
    type Output = GridVec2;
    fn sub(self, d: GridVec2) -> GridVec2 {
        return GridVec2 {
            row: self.row - d.row,
            col: self.col - d.col,
        };
    }
}

pub type GridPos = GridVec2;

pub type GridDelta = GridVec2;

/// A Tile.
#[derive(Clone, Copy, Debug, Component, Eq, PartialEq)]
pub enum Tile {
    Floor,
    Wall,
    MazeStart,
    MazeEnd,
}

impl Tile {
    fn index_in_tile_spritesheet(self) -> usize {
        return match self {
            Tile::Floor => 0,
            Tile::Wall => 1,
            Tile::MazeStart => 2,
            Tile::MazeEnd => 3,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GridError {
    /// `bottom` > `top`.
    InvertedRows { bottom: i32, top: i32 },
    /// `left` > `right`.
    InvertedCols { left: i32, right: i32 },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvertedRows { bottom, top } => write!(f, "bottom({bottom}) > top({top})"),
            Self::InvertedCols { left, right } => write!(f, "left({left}) > right({right})"),
        }
    }
}

/// A 2D grid of tiles, representing the layout of the level.
#[derive(Debug)]
pub struct TileGrid {
    // row/col index of the grid's bounds:
    // bottom <= row <= top, left <= col <= right
    bottom: i32,
    left: i32,
    top: i32,
    right: i32,
    // (row, col) -> tiles[row*cols + col]
    tiles: Vec<Tile>,
}

impl TileGrid {
    fn new(bottom: i32, left: i32, top: i32, right: i32) -> Result<Self, GridError> {
        if bottom > top {
            return Err(GridError::InvertedRows { bottom, top });
        }
        if left > right {
            return Err(GridError::InvertedCols { left, right });
        }

        let rows = (top - bottom + 1) as usize;
        let cols = (right - left + 1) as usize;
        let grid = TileGrid {
            bottom,
            left,
            top,
            right,
            tiles: vec![Tile::Floor; rows * cols],
        };

        return Ok(grid);
    }

    fn rows(&self) -> usize {
        return (self.top - self.bottom + 1) as usize;
    }

    fn cols(&self) -> usize {
        return (self.right - self.left + 1) as usize;
    }

    fn contains(&self, pos: GridVec2) -> bool {
        return (self.bottom..=self.top).contains(&pos.row)
            && (self.left..=self.right).contains(&pos.col);
    }

    /// `GridPos` -> index into `tiles`, or `None` (out of bounds).
    fn flat_index(&self, pos: GridVec2) -> Option<usize> {
        if !self.contains(pos) {
            return None;
        }
        let row = (pos.row - self.bottom) as usize;
        let col = (pos.col - self.left) as usize;
        return Some(row * self.cols() + col);
    }

    /// Returns the tile at a `GridPos`, or `None` (out of bounds).
    fn get(&self, pos: GridVec2) -> Option<&Tile> {
        return self.flat_index(pos).map(|i| &self.tiles[i]);
    }

    /// Returns a mutable reference to the tile at a `GridPos`, or `None` (out of bounds).
    fn get_mut(&mut self, pos: GridVec2) -> Option<&mut Tile> {
        let i = self.flat_index(pos)?;
        return Some(&mut self.tiles[i]);
    }

    /// Iterator for looping through all tiles.
    fn iter(&self) -> impl Iterator<Item = (usize, GridVec2, Tile)> + '_ {
        return (self.bottom..=self.top).flat_map(move |row| {
            (self.left..=self.right).map(move |col| {
                let pos = GridVec2 { row, col };
                let index = self.flat_index(pos).unwrap();
                (index, pos, self.tiles[index])
            })
        });
    }
}

impl TileGrid {
    fn for_level(lvl: Level) -> Self {
        match lvl {
            Level::_0 => Self::lvl_0(),
            Level::_1 => Self::lvl_1(),
            Level::_2 => Self::lvl_2(),
        }
    }

    fn lvl_0() -> Self {
        let grid = TileGrid::new(-5, -9, 5, 9).unwrap();

        return grid;
    }

    fn lvl_1() -> Self {
        let mut grid = TileGrid::new(-1, -5, 1, 5).unwrap();
        let bottom = grid.bottom;
        let top = grid.top;
        let left = grid.left;
        let right = grid.right;
        for c in grid.left..=grid.right {
            grid[GridVec2::new(bottom, c)] = Tile::Wall;
            grid[GridVec2::new(top, c)] = Tile::Wall;
        }
        for r in grid.bottom..=grid.top {
            grid[GridVec2::new(r, left)] = Tile::Wall;
            grid[GridVec2::new(r, right)] = Tile::Wall;
        }
        grid[GridVec2::new(0, -4)] = Tile::MazeStart;
        grid[GridVec2::new(0, 4)] = Tile::MazeEnd;

        return grid;
    }

    fn lvl_2() -> Self {
        let mut grid = TileGrid::new(-5, -9, 5, 9).unwrap();
        let bottom = grid.bottom;
        let top = grid.top;
        let left = grid.left;
        let right = grid.right;
        for c in grid.left..=grid.right {
            grid[GridVec2::new(bottom, c)] = Tile::Wall;
            grid[GridVec2::new(top, c)] = Tile::Wall;
        }
        for r in grid.bottom..=grid.top {
            grid[GridVec2::new(r, left)] = Tile::Wall;
            grid[GridVec2::new(r, right)] = Tile::Wall;
        }
        grid[GridVec2::new(-4, -8)] = Tile::MazeStart;
        grid[GridVec2::new(4, 8)] = Tile::MazeEnd;

        grid[GridVec2::new(-2, -2)] = Tile::Wall;
        grid[GridVec2::new(-2, 1)] = Tile::Wall;
        grid[GridVec2::new(-1, 1)] = Tile::Wall;
        grid[GridVec2::new(0, 1)] = Tile::Wall;
        grid[GridVec2::new(1, 1)] = Tile::Wall;
        grid[GridVec2::new(2, 1)] = Tile::Wall;
        grid[GridVec2::new(0, 2)] = Tile::Wall;
        grid[GridVec2::new(0, 3)] = Tile::Wall;
        grid[GridVec2::new(1, 4)] = Tile::Wall;
        grid[GridVec2::new(2, 4)] = Tile::Wall;

        return grid;
    }
}

impl Index<GridVec2> for TileGrid {
    type Output = Tile;

    fn index(&self, pos: GridVec2) -> &Tile {
        return self
            .get(pos)
            .unwrap_or_else(|| panic!("{pos:?} out of grid bounds"));
    }
}

impl IndexMut<GridVec2> for TileGrid {
    fn index_mut(&mut self, pos: GridVec2) -> &mut Tile {
        return self
            .get_mut(pos)
            .unwrap_or_else(|| panic!("{pos:?} out of grid bounds"));
    }
}

#[derive(Component)]
#[require(Transform, Visibility)]
struct LevelRoot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, States)]
pub enum Level {
    _0,
    _1,
    _2,
}

impl Level {
    const ALL: [Level; 3] = [Level::_0, Level::_1, Level::_2];
}

// fn on_maze_end_reached(mut next: ResMut<NextState<Level>>) {
//     next.set(Level::_0);
// }

/// All Level(s) used in-game.
#[derive(Resource)]
pub struct Levels(HashMap<Level, TileGrid>);

impl Levels {
    pub fn get(&self, lvl: Level) -> &TileGrid {
        return &self.0[&lvl];
    }
}

impl FromWorld for Levels {
    fn from_world(_: &mut World) -> Self {
        let mut levels = HashMap::new();
        for lvl in Level::ALL {
            levels.insert(lvl, TileGrid::for_level(lvl));
        }

        return Levels(levels);
    }
}
