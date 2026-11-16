use std::fmt;

use bevy::log::info;
use bevy::prelude::{
    App, ButtonInput, Commands, Component, Fixed, FixedUpdate, IntoScheduleConfigs, KeyCode, Name,
    Plugin, Query, Res, Sprite, Startup, State, Time, Timer, TimerMode, Transform, Update, With,
    RunFixedMainLoop, RunFixedMainLoopSystems, Resource, IVec2, ResMut,
};
use bevy::sprite::Anchor;
use core::time::Duration;

pub struct PlayerPlugin;

use crate::global;
use crate::level::GridPos;
use crate::level::Level;
use crate::level::Levels;
use crate::level::Tile;
use crate::level::TileGrid;
use crate::spritesheet::SpriteSheet;
use crate::spritesheet::SpriteSheets;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(
                global::SIMULATION_INTERVAL_SECONDS,
            ))
            .add_systems(Startup, spawn_player)
            // per render frame, but before the FixedUpdate if they happen to coincide
            .add_systems(RunFixedMainLoop, collect_input.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop))
            // per sim frame
            .init_resource::<SimTick>()
            .add_systems(FixedUpdate, (sample_input, simulate).chain())
            // .add_systems(Update, handle_player_input)
            // per render frame
            .add_systems(Update, animate_player);
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
            // prev_pos: GridPos::new(-1, -1), // debug
            facing: Facing::Right,
            prev_facing: Facing::Down,
        },
        InputIntent::default(),
        InputCommand::default(),
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

fn resolve_move(
    pos: GridPos,
    cmd: &InputCommand,
    grid: &TileGrid,
) -> Option<(GridPos, Facing)> {
    let row_delta= cmd.delta.x; // TODO: fix this?
    let col_delta= cmd.delta.y;
    if row_delta == 0 && col_delta == 0 {
        return None;
    }

   let vert = if row_delta != 0 {
        let facing = if row_delta < 0 { Facing::Down } else { Facing::Up };
        Some((pos.translate(row_delta, 0), facing))
    } else {
        None
    };
   let horiz = if col_delta != 0 {
        let facing = if col_delta < 0 { Facing::Left } else { Facing::Right };
        Some((pos.translate(0, col_delta), facing))
    } else {
        None
    };

    { // allow diag
        if let (Some((row, row_facing)), Some((col, col_facing))) = (vert, horiz) {
            let diag = pos.translate(row_delta, col_delta);
            let row_walkable = grid[row] != Tile::Wall;
            let col_walkable = grid[row] != Tile::Wall;
            let diag_walkable = grid[diag] != Tile::Wall;

            if diag_walkable && (row_walkable || col_walkable) {
                let facing = match cmd.recent {
                    Some(facing) => facing,
                    None => Facing::Down,
                };
                // let facing = cmd.recent.filter(|f| *f == col_facing || *f == row_facing).unwrap_or(hf);
                return Some((diag, facing));
            }
        }
    }

    let recent_is_vert = matches!(cmd.recent, Some(Facing::Up | Facing::Down));
    let order = if recent_is_vert {
        [vert, horiz]
    } else {
        [horiz, vert]
    };
    order.into_iter().flatten().find(|(pos, _)| grid[*pos] != Tile::Wall)
}

fn simulate(
    mut tick: ResMut<SimTick>,
    current_level: Res<State<Level>>,
    levels: Res<Levels>,
    mut query: Query<(&InputCommand, &mut SimulationState), With<Player>>,
) {
    let Ok((cmd, mut sim_state)) = query.single_mut() else {
        return;
    };

    info!("sim: cmd={}", cmd);

    sim_state.prev_pos = sim_state.pos;
    sim_state.prev_facing = sim_state.facing;

    let lvl = *current_level.get();
    let grid = levels.get(lvl);
    match resolve_move(sim_state.pos, cmd, grid) {
        Some((pos, facing)) => {
            sim_state.pos = pos;
            sim_state.facing = facing;
        },
        None => {
            if let Some(facing) = cmd.recent {
                sim_state.facing = facing;
            };
        },
    }

    tick.0 = tick.0 + 1;

    // let user_input: Option<Facing> = buffer.tapped.take().or(buffer.held);
    // let intent: Option<MoveTo> = match user_input {
    //     Some(facing) => {
    //         let (delta_row, delta_col) = facing.delta_row_and_col();
    //         Some(MoveTo {
    //             pos: sim_state.pos.translate(delta_row, delta_col),
    //             facing: facing,
    //         })
    //     }
    //     None => None,
    // };

    // match intent {
    //     Some(intent) => {
    //         info!("saw({})", intent);


    //         let is_walkable = grid[intent.pos] != Tile::Wall;
    //         if is_walkable {
    //             sim_state.prev_pos = sim_state.pos;
    //             sim_state.prev_facing = sim_state.facing;

    //             sim_state.pos = intent.pos;
    //             sim_state.facing = intent.facing;
    //         } else {
    //             sim_state.prev_pos = sim_state.pos;
    //             sim_state.prev_facing = sim_state.facing;
    //         }
    //         info!(
    //             "sim(MoveTo): prev_pos=(r({}), c({})); pos=(r({}), c({})); prev_facing={}, facing={}",
    //             sim_state.prev_pos.row,
    //             sim_state.prev_pos.col,
    //             sim_state.pos.row,
    //             sim_state.pos.col,
    //             sim_state.prev_facing,
    //             sim_state.facing
    //         );

    //         return;
    //     }
    //     None => {
    //         info!("saw(None)");
    //         sim_state.prev_pos = sim_state.pos;
    //         sim_state.prev_facing = sim_state.facing;
    //         info!(
    //             "sim(None): prev_pos=(r({}), c({})); pos=(r({}), c({})); prev_facing={}, facing={}",
    //             sim_state.prev_pos.row,
    //             sim_state.prev_pos.col,
    //             sim_state.pos.row,
    //             sim_state.pos.col,
    //             sim_state.prev_facing,
    //             sim_state.facing
    //         );
    //     }
    // };
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
        // info!("animate: transform.set(x={}, y={})", xy.x, xy.y);
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
    const ALL: [Facing; 4] = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];

    fn delta(&self) -> IVec2 {
        match self {
            Facing::Down => IVec2::new(-1, 0),
            Facing::Left => IVec2::new(0, -1),
            Facing::Up => IVec2::new(1, 0),
            Facing::Right => IVec2::new(0, 1),
        }
    }

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

    fn key_code(self) -> KeyCode {
        match self {
            Facing::Down => KeyCode::ArrowDown,
            Facing::Left => KeyCode::ArrowLeft,
            Facing::Up => KeyCode::ArrowUp,
            Facing::Right => KeyCode::ArrowRight,
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

#[derive(Default, Debug)]
struct KeyMemory {
    not_yet_sampled: bool,
    last_seq: u32,
}

#[derive(Component, Debug, Default)]
struct InputIntent {
    up: KeyMemory,
    down: KeyMemory,
    left: KeyMemory,
    right: KeyMemory,
    seq: u32, // monotonic counter
}

impl InputIntent {
    // fn key(&self, dir: Facing) -> KeyMemory {
    //     match dir {
    //         Facing::Down => self.down,
    //         Facing::Left => self.left,
    //         Facing::Up => self.up,
    //         Facing::Right => self.right,
    //     }
    // }

    // fn key_mut(&mut self, dir: Facing) -> &mut KeyMemory {
    //     match dir {
    //         Facing::Up => &mut self.up,
    //         Facing::Down => &mut self.down,
    //         Facing::Left => &mut self.left,
    //         Facing::Right => &mut self.right,
    //     }
    // }

    fn record(&mut self, dir: Facing) {
        self.seq += 1; // self.seq = self.seq.wrapping_add(1);
        match dir {
            Facing::Down => {
                self.down.not_yet_sampled= true;
                self.down.last_seq = self.seq;
            },
            Facing::Left => {
                self.left.not_yet_sampled= true;
                self.left.last_seq = self.seq;
            },
            Facing::Up => {
                self.up.not_yet_sampled= true;
                self.up.last_seq = self.seq;
            },
            Facing::Right => {
                self.right.not_yet_sampled= true;
                self.right.last_seq = self.seq;
            },
        }
        // info!("collect_input: record: {}(not_yet_sampled: true, last_seq={})", dir, self.seq);
    }

    /// Event: Sample, once per sim frame. `is_down` is `ButtonInput::pressed`.
    ///
    /// A key is active if it is down now or was pressed since the last sample.
    /// Opposite directions resolve as "last input wins" (SOCD).
    fn sample(&mut self, sim_tick: u32, down_pressed: bool, left_pressed: bool, up_pressed: bool, right_pressed: bool) -> InputCommand {

        // keys
        let down = down_pressed || self.down.not_yet_sampled;
        let left = left_pressed || self.left.not_yet_sampled;
        let up = up_pressed || self.up.not_yet_sampled;
        let right = right_pressed || self.right.not_yet_sampled;

        // clean up SOCD (Simultaneous Opposing Cardinal Directions)
        let row_moved: Option<(Facing, u32)> = if down && up {
            if self.down.last_seq > self.up.last_seq {
                Some((Facing::Down, self.down.last_seq))
            } else {
                Some((Facing::Up, self.up.last_seq))
            } // two last_seq cannot be the same, because we how we increase them after each check
              // in collect_input()
        } else if down {
                Some((Facing::Down, self.down.last_seq))
        } else if up {
                Some((Facing::Up, self.up.last_seq))
        } else {
            None
        };
        let col_moved: Option<(Facing, u32)> = if left && right {
            if self.left.last_seq > self.right.last_seq {
                Some((Facing::Left, self.left.last_seq))
            } else {
                Some((Facing::Right, self.right.last_seq))
            } // two last_seq cannot be the same, because we how we increase them after each check
              // in collect_input()
        } else if left {
                Some((Facing::Left, self.left.last_seq))
        } else if right {
                Some((Facing::Right, self.right.last_seq))
        } else {
            None
        };

        // sort
        let command = match (row_moved, col_moved) {
            (Some((row_direction, row_last_seq)), Some((col_direction, col_last_seq))) => {
                let recent: Option<Facing> = if row_last_seq > col_last_seq {
                    Some(row_direction)
                } else {
                    Some(col_direction)
                };

                InputCommand{
                    sim_tick: sim_tick,
                    delta: row_direction.delta() + col_direction.delta(),
                    recent: recent,
                }
            },
            (Some((row_direction, row_last_seq)), None) => {
                InputCommand{
                    sim_tick: sim_tick,
                    delta: row_direction.delta(),
                    recent: Some(row_direction),
                }
            },
            (None, Some((col_direction, col_last_seq))) => {
                InputCommand{
                    sim_tick: sim_tick,
                    delta: col_direction.delta(),
                    recent: Some(col_direction),
                }
            },
            (None, None) => {
               InputCommand{
                    sim_tick: sim_tick,
                    delta: IVec2::new(0, 0),
                    recent: None,
                }
            },
        };

        self.down.not_yet_sampled = false;
        self.left.not_yet_sampled = false;
        self.up.not_yet_sampled = false;
        self.right.not_yet_sampled = false;

        return command;
    }
}

fn collect_input(
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut InputIntent, With<Player>>,
) {
    let Ok(mut intent) = query.single_mut() else {
        return;
    };

    for dir in Facing::ALL {
        if input.just_pressed(dir.key_code()) {
            intent.record(dir);
        }
    }
}

#[derive(Resource, Debug, Default)]
struct SimTick(u32);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct InputCommand {
    /// The sim frame this command is for.
    sim_tick: u32,
    /// first: -1 = down, 0 = none, +1 = up; second: -1 = left, 0 = none, +1 = right.
    delta: IVec2,
    /// Most recently pressed active direction
    recent: Option<Facing>,
}

impl fmt::Display for InputCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{ sim_tick={}, delta=(r{}, c{}), recent={:?} }}", self.sim_tick, self.delta.x, self.delta.y, self.recent)
    }
}

fn sample_input(
    tick: Res<SimTick>,
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut InputIntent, &mut InputCommand), With<Player>>,
) {
    let Ok((mut intent, mut cmd)) = query.single_mut() else {
        return;
    };

    let down_pressed = input.pressed(Facing::Down.key_code());
    let left_pressed = input.pressed(Facing::Left.key_code());
    let up_pressed = input.pressed(Facing::Up.key_code());
    let right_pressed = input.pressed(Facing::Right.key_code());
    *cmd = intent.sample(tick.0, down_pressed, left_pressed, up_pressed, right_pressed);
    // info!("command={}", *cmd);
}
