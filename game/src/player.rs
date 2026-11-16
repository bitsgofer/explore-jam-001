use std::fmt;

use bevy::log::info;
use bevy::prelude::{
    App, ButtonInput, Commands, Component, Fixed, FixedUpdate, IntoScheduleConfigs, KeyCode, Name,
    Plugin, Query, Res, Sprite, Startup, State, Time, Timer, TimerMode, Transform, Update, With,
};
use bevy::sprite::Anchor;
use core::time::Duration;

pub struct PlayerPlugin;

use crate::global;
use crate::level::GridPos;
use crate::level::Level;
use crate::level::Levels;
use crate::level::Tile;
use crate::spritesheet::SpriteSheet;
use crate::spritesheet::SpriteSheets;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            // .add_systems(Update, handle_player_input)
            .add_systems(Update, animate_player)
            .insert_resource(Time::<Fixed>::from_seconds(
                global::SIMULATION_INTERVAL_SECONDS,
            ));
        // .add_systems(FixedUpdate, simulate);
    }
}

fn spawn_player(mut commands: Commands, sheets: Res<SpriteSheets>) {
    let sheet = sheets.get(SpriteSheet::Player);
    let pos = GridPos::new(0, 0);
    let animation_index: usize = 0;
    commands.spawn((
        Player,
        pos,
        sheet.sprite(animation_index),
        Transform::from_translation(pos.to_xy().extend(global::PLAYER_Z)),
        Anchor::BOTTOM_LEFT,
        SimulationState {
            pos: pos,
            prev_pos: pos,
            // prev_pos: GridPos::new(-1, 0), // debug
            facing: Facing::Down,
            prev_facing: Facing::Down,
        },
        Buffer {
            held: None,
            tapped: None,
        },
        AnimationState {
            // will get modified in animate_player()
            moving: false,
            facing: Facing::Down,
            index: 15 as usize,
            timer: Timer::from_seconds(
                global::PLAYER_ANIMATION_FRAME_DURATION_SECONDS,
                TimerMode::Repeating,
            ),
        },
        Name::new(format!("Player({}, {})", pos.row, pos.col)),
    ));
}

fn handle_player_input(
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Buffer, With<Player>>,
) {
    let Ok(mut buffer) = query.single_mut() else {
        return;
    };

    if input.pressed(KeyCode::ArrowDown) {
        buffer.held = Some(Facing::Down);
    } else if input.pressed(KeyCode::ArrowLeft) {
        buffer.held = Some(Facing::Left);
    } else if input.pressed(KeyCode::ArrowUp) {
        buffer.held = Some(Facing::Up);
    } else if input.pressed(KeyCode::ArrowRight) {
        buffer.held = Some(Facing::Right);
    } else {
        buffer.held = None;
    };
    info!("still held this render frame: {:?}", buffer.held);

    if input.just_pressed(KeyCode::ArrowDown) {
        buffer.tapped = Some(Facing::Down);
    } else if input.just_pressed(KeyCode::ArrowLeft) {
        buffer.tapped = Some(Facing::Left);
    } else if input.just_pressed(KeyCode::ArrowUp) {
        buffer.tapped = Some(Facing::Up);
    } else if input.just_pressed(KeyCode::ArrowRight) {
        buffer.tapped = Some(Facing::Right);
    } else {
        buffer.tapped = None;
    };
    info!("just pressed this render frame: {:?}", buffer.tapped);

    return;
}

fn simulate(
    current_level: Res<State<Level>>,
    levels: Res<Levels>,
    mut query: Query<(&mut Buffer, &mut SimulationState), With<Player>>,
) {
    let Ok((mut buffer, mut sim_state)) = query.single_mut() else {
        return;
    };

    let user_input: Option<Facing> = buffer.tapped.take().or(buffer.held);
    let intent: Option<MoveTo> = match user_input {
        Some(facing) => {
            let (delta_row, delta_col) = facing.delta_row_and_col();
            Some(MoveTo {
                pos: sim_state.pos.translate(delta_row, delta_col),
                facing: facing,
            })
        }
        None => None,
    };

    match intent {
        Some(intent) => {
            info!("saw({})", intent);

            let lvl = *current_level.get();
            let grid = levels.get(lvl);

            let is_walkable = grid[intent.pos] != Tile::Wall;
            if is_walkable {
                sim_state.prev_pos = sim_state.pos;
                sim_state.prev_facing = sim_state.facing;

                sim_state.pos = intent.pos;
                sim_state.facing = intent.facing;
            } else {
                sim_state.prev_pos = sim_state.pos;
                sim_state.prev_facing = sim_state.facing;
            }
            info!(
                "sim(MoveTo): prev_pos=(r({}), c({})); pos=(r({}), c({})); prev_facing={}, facing={}",
                sim_state.prev_pos.row,
                sim_state.prev_pos.col,
                sim_state.pos.row,
                sim_state.pos.col,
                sim_state.prev_facing,
                sim_state.facing
            );

            return;
        }
        None => {
            info!("saw(None)");
            sim_state.prev_pos = sim_state.pos;
            sim_state.prev_facing = sim_state.facing;
            info!(
                "sim(None): prev_pos=(r({}), c({})); pos=(r({}), c({})); prev_facing={}, facing={}",
                sim_state.prev_pos.row,
                sim_state.prev_pos.col,
                sim_state.pos.row,
                sim_state.pos.col,
                sim_state.prev_facing,
                sim_state.facing
            );
        }
    };
}

fn animate_player(
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<
        (
            &mut Transform,
            &mut AnimationState,
            &mut Sprite,
            &SimulationState,
        ),
        With<Player>,
    >,
) {
    let Ok((mut transform, mut animation, mut sprite, sim_state)) = query.single_mut() else {
        return;
    };

    // animate position
    {
        let frac = fixed_time.overstep_fraction();
        let xy = sim_state
            .prev_pos
            .to_xy()
            .lerp(sim_state.pos.to_xy(), frac)
            .round();
        transform.translation.x = xy.x;
        transform.translation.y = xy.y;
        info!("animate: transform.set(x={}, y={})", xy.x, xy.y);
    }

    // animate the sprite
    // CONTROL: dependent on simulate() advancing the .facing/.pos
    {
        let desired_facing: Facing = sim_state.facing;
        let desired_moving: bool = sim_state.pos != sim_state.prev_pos;
        // info!("animate: desired_facing={}, desired_moving={}; anim.facing={}, anim.moving={}", desired_facing, desired_moving, animation.facing, animation.moving);

        let animation_col = animation.index % global::MAX_FRAMES_PER_ANIMATION;
        if desired_moving != animation.moving || desired_facing != animation.facing {
            animation.moving = desired_moving;
            animation.facing = desired_facing;
            animation.index = animation.facing.row_index_in_player_spritesheet()
                * global::MAX_FRAMES_PER_ANIMATION;
            animation.timer.reset();
            // info!("animate: reset; facing={}, moving={}, index={}", animation.facing, animation.moving, animation.index);
        } else if desired_moving {
            animation.timer.tick(time.delta());
            animation.index = animation.facing.row_index_in_player_spritesheet()
                * global::MAX_FRAMES_PER_ANIMATION
                + (animation_col + animation.timer.times_finished_this_tick() as usize)
                    % global::MAX_FRAMES_PER_ANIMATION;
            // info!("animate: continue; facing={}, moving={}, index={}", animation.facing, animation.moving, animation.index);
        }

        let atlas = match sprite.texture_atlas.as_mut() {
            Some(a) => a,
            None => return,
        };
        atlas.index = animation.index;
        // info!("animate: atlas.index={}", animation.index);
    }

    return;
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct SimulationState {
    pos: GridPos,
    prev_pos: GridPos,
    facing: Facing,
    prev_facing: Facing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Down,
    Left,
    Up,
    Right,
}

impl Facing {
    fn delta_row_and_col(&self) -> (i32, i32) {
        match self {
            Facing::Down => (-1, 0),
            Facing::Left => (0, -1),
            Facing::Up => (1, 0),
            Facing::Right => (0, 1),
        }
    }

    fn row_index_in_player_spritesheet(&self) -> usize {
        match self {
            Facing::Down => 0,
            Facing::Left => 1,
            Facing::Up => 2,
            Facing::Right => 3,
        }
    }
}

impl fmt::Display for Facing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Facing::Down => write!(f, "Down"),
            Facing::Left => write!(f, "Left"),
            Facing::Up => write!(f, "Up"),
            Facing::Right => write!(f, "Right"),
        }
    }
}

/// Player's intent (i.e move to (row, col) & face Down/Left/Up/Right
#[derive(Debug, Copy, Clone)]
struct MoveTo {
    pos: GridPos,
    facing: Facing,
}

impl fmt::Display for MoveTo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MoveTo{{(r({}), c({})), facing({})}}",
            self.pos.row, self.pos.col, self.facing
        )
    }
}

/// Player's input buffer.
/// Usage: buffer.tapped.take().or(buffer.held)
#[derive(Debug, Component)]
struct Buffer {
    /// Direction currently held down (level). Is true from 1st render frame
    /// after key is pressed, until first render frame after key is released.
    held: Option<Facing>,
    /// Latest fresh press (edge). Is true from 1st render frame
    /// after key is pressed and is cleared after either:
    /// - 2nd render frame after key is pressed, if sim freq >= render freq
    /// - 1st render frame AFTER the 1st sim frame after key is pressed, if sim freq < render freq
    tapped: Option<Facing>,
}

#[derive(Component)]
struct AnimationState {
    moving: bool,
    facing: Facing,
    index: usize,
    timer: Timer,
}
