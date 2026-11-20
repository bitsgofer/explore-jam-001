use std::fmt;

use bevy::log::info;
use bevy::prelude::{
    App, ButtonInput, Commands, Component, Fixed, FixedUpdate, IVec2, IntoScheduleConfigs, KeyCode,
    Name, Plugin, Query, Res, ResMut, Resource, RunFixedMainLoop, RunFixedMainLoopSystems, Sprite,
    Startup, State, Time, Timer, TimerMode, Transform, Update, With,
};

use bevy::sprite::Anchor;

use crate::global;
use crate::level::{GridPos, Level, Levels, Tile, TileGrid};
use crate::spritesheet::{SpriteSheet, SpriteSheets};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .insert_resource(SimStepNo::new(1))
            .insert_resource(Time::<Fixed>::from_seconds(
                global::SIMULATION_INTERVAL_SECONDS,
            ))
            .add_systems(
                RunFixedMainLoop,
                collect_input.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            ) // per render frame, but before FixedUpdate (sim) // !IMPORTANT
            .add_systems(FixedUpdate, (compute_sim_changes, simulate).chain()) // per sim step
            .add_systems(Update, animate_player); // per render frame
        return;
    }
}

fn spawn_player(mut commands: Commands, sheets: Res<SpriteSheets>) {
    let sheet = sheets.get(SpriteSheet::Player);
    let init_pos = GridPos::new(0, 0);
    commands.spawn((
        Player,
        init_pos,
        sheet.sprite(0 as usize),
        Transform::from_translation(init_pos.to_xy().extend(global::PLAYER_Z)),
        Anchor::BOTTOM_LEFT,
        Name::new(format!("Player")),
        InputBuffer::default(),
        InputCommand::default(),
        SimulationState::default(),
        // SimulationState { // debug
        //     prev_pos: GridPos::new(1, 0),
        //     pos: init_pos,
        //     prev_facing: Facing::Down,
        //     facing: Facing::Down,
        // },
        AnimationState::default_with_anim_duration(global::PLAYER_ANIMATION_FRAME_DURATION_SECONDS),
    ));
}

/// Every render frame, look at what key(s) player pressed & when (render frame
/// sequence number) => store into InputBuffer.
///
/// If the same key is already pressed, then last write wins (only change the
/// sequence number). This happens when press + release + press very quickly.
///
/// This allow the sim to see all keys pressed since it last run.
/// => handle player's intent/input more accurately.
///
/// !IMPORTANT: The sim will also clear the buffer after reading.
fn collect_input(
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut InputBuffer, With<Player>>,
) {
    let Ok(mut buffer) = query.single_mut() else {
        return;
    };

    for key in Facing::ALL.map(Facing::key_codes).concat() {
        if input.just_pressed(key) {
            buffer.record(key);
        }
    }
}

/// Calculate the player's input towards the simulation.
///
/// In a client-server setup, this will be the client code.
fn compute_sim_changes(
    sim_step: Res<SimStepNo>,
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut InputBuffer, &mut InputCommand), With<Player>>,
) {
    let Ok((mut input_buffer, mut cmd)) = query.single_mut() else {
        return;
    };

    let (delta, facing) = player_intent(&input_buffer, input);
    // info!("compute_sim_changes: delta={}, facing={:?}", delta, facing);

    input_buffer.clear();

    let command = InputCommand{
        sim_step: *sim_step,
        delta: IVec2::new(delta.y, delta.x), // TODO: switch to delta type, instead of reversing to get (dr, dc)
        facing,
    };
    *cmd = command;
    // info!("compute_sim_changes: send cmd={}", *cmd);
}

/// Called every RenderFrame that runs together (and before) a SimStep,
/// this fn computes the next delta(row, col) && Facing direction,
/// which we will need for the next simulation step.
///
/// It use the buffered input (since the last SimStep) + the current input
/// seen this RenderFrame for this calculation.
fn player_intent(buffer: &InputBuffer, input: Res<ButtonInput<KeyCode>>) -> (IVec2, Option<Facing>) {
    // keys pressed in this RenderFrame
    let pressed_down = if input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::KeyJ) { true } else { false };
    let pressed_left = if input.pressed(KeyCode::ArrowLeft) || input.pressed(KeyCode::KeyA) || input.pressed(KeyCode::KeyH) { true } else { false };
    let pressed_up = if input.pressed(KeyCode::ArrowUp) || input.pressed(KeyCode::KeyW) || input.pressed(KeyCode::KeyK) { true } else { false };
    let pressed_right = if input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::KeyD) || input.pressed(KeyCode::KeyL) { true } else { false };

    // combine pressed_DIRECTION (in this RenderFrame) with the input
    // bufered since the last SimStep (it was not yet handled)
    // => direction that the player intended to move
    let down = pressed_down || buffer.pressed_down;
    let left = pressed_left || buffer.pressed_left;
    let up = pressed_up || buffer.pressed_up;
    let right = pressed_right || buffer.pressed_right;

    // translate keys pressed -> delta in row && col, i.e (-1, 0, +1)
    //
    // we also handle SOCD (Simultaneous Opposing Cardinal Directions),
    // picking only one direction between lef/right or up/down (the latest)
    let delta_row: Option<(i32, Facing, RenderFrameNo)> = if down && up {
        if buffer.pressed_down_at.0 > buffer.pressed_up_at.0 {
            Some((-1, Facing::Down, buffer.pressed_down_at))
        } else {
            Some((1, Facing::Up, buffer.pressed_up_at))
        }
    } else if down {
        Some((-1, Facing::Down, buffer.pressed_down_at))
    } else if up {
        Some((1, Facing::Up, buffer.pressed_up_at))
    } else {
        None
    };
    let delta_col: Option<(i32, Facing, RenderFrameNo)> = if left && right {
        if buffer.pressed_left_at.0 > buffer.pressed_right_at.0 {
            Some((-1, Facing::Left, buffer.pressed_left_at))
        } else {
            Some((1, Facing::Right, buffer.pressed_right_at))
        }
    } else if left {
        Some((-1, Facing::Left, buffer.pressed_left_at))
    } else if right {
        Some((1, Facing::Right, buffer.pressed_right_at))
    } else {
        None
    };

    // aggregate into delta(row, col)
    // and translate the latest direction key pressed -> Facing direction
    match (delta_row, delta_col) {
        (Some((dr, r_facing, r_pressed_at)), Some((dc, c_facing, c_pressed_at))) => {
            let facing = if r_pressed_at.0 > c_pressed_at.0 {
                r_facing
            } else {
                c_facing
            };

            (IVec2::new(dr, dc), Some(facing))
        }
        (Some((dr, r_facing, r_pressed_at)), None) => {
            (IVec2::new(dr, 0), Some(r_facing))
        }
        (None, Some((dc, c_facing, c_pressed_at))) => {
            (IVec2::new(0, dc), Some(c_facing))
        }
        (None, None) => {
            (IVec2::new(0, 0), None)
        }
    }
}

/// Run simulation.
///
/// In a client-server setup, this will be the server code.
fn simulate(
    mut sim_step: ResMut<SimStepNo>,
    level: Res<State<Level>>,
    levels: Res<Levels>,
    mut query: Query<(&InputCommand, &mut SimulationState), With<Player>>,
) {
    let Ok((cmd, mut sim)) = query.single_mut() else {
        return;
    };
    // info!("sim: received cmd={}", cmd);

    // advance sim state (current -> prev)
    sim.prev_pos = sim.pos;
    sim.prev_facing = sim.facing;
    // advance sim step
    sim_step.0 = sim_step.0 + 1;

    // run the actual sim
    let grid = levels.get(*level.get());
    simulate_player(&mut sim, cmd, grid);

    return;
}

fn simulate_player(sim: &mut SimulationState, cmd: &InputCommand, grid: &TileGrid) {
    let changes = player_changes(sim.pos, cmd, grid);
    match changes {
        Some((pos, facing)) => {
            sim.pos = pos;
            sim.facing = facing;
        },
        None => {
            if let Some(facing) = cmd.facing {
                sim.facing = facing;
            };
        },
    }
}

/// Simulate where the player will be, based on the level's map.
fn player_changes(pos: GridPos, cmd: &InputCommand, grid: &TileGrid) -> Option<(GridPos, Facing)> {
    let delta_row = cmd.delta.y;
    let delta_col = cmd.delta.x;

    if delta_row == 0 && delta_col == 0 {
        return None;
    }

    let vert = if delta_row != 0 {
        let facing = if delta_row < 0 {
            Facing::Down
        } else {
            Facing::Up
        };
        Some((pos.translate(delta_row, 0), facing))
    } else {
        None
    };
    let horiz = if delta_col != 0 {
        let facing = if delta_col < 0 {
            Facing::Left
        } else {
            Facing::Right
        };
        Some((pos.translate(0, delta_col), facing))
    } else {
        None
    };

        // allow diag
        if let (Some((row, row_facing)), Some((col, col_facing))) = (vert, horiz) {
            let diag = pos.translate(delta_row, delta_col);
            let row_walkable = grid[row] != Tile::Wall;
            let col_walkable = grid[row] != Tile::Wall;
            let diag_walkable = grid[diag] != Tile::Wall;

            if diag_walkable && (row_walkable || col_walkable) {
                let facing = match cmd.facing {
                    Some(facing) => facing,
                    None => Facing::Down,
                };
                // let facing = cmd.recent.filter(|f| *f == col_facing || *f == row_facing).unwrap_or(hf);
                return Some((diag, facing));
            }
        }

    let recent_is_vert = matches!(cmd.facing, Some(Facing::Up | Facing::Down));
    let order = if recent_is_vert {
        [vert, horiz]
    } else {
        [horiz, vert]
    };
    order
        .into_iter()
        .flatten()
        .find(|(pos, _)| grid[*pos] != Tile::Wall)
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
    let Ok((mut transform, mut anim, mut sprite, sim)) = query.single_mut() else {
        return;
    };

    interpolate_xy(fixed_time, &mut transform, sim); // TODO: What happens if we use time instead of fixed_time?
    update_sprite(time, sim, &mut anim, &mut sprite);

    return;
}

fn interpolate_xy(fixed_time: Res<Time<Fixed>>, transform: &mut Transform, sim: &SimulationState) {
    let frac = fixed_time.overstep_fraction();
    let from = sim.prev_pos.to_xy();
    let to = sim.pos.to_xy();

    transform.translation = from.lerp(to, frac).round().extend(global::PLAYER_Z);
    // info!("animate: transform.translation(x={}, y={})", transform.translation.x, transform.translation.y);
}

fn update_sprite(
    time: Res<Time>,
    sim: &SimulationState,
    anim: &mut AnimationState,
    sprite: &mut Sprite,
) {
    // read sim states
    let next_facing: Facing = sim.facing;
    let next_moving: bool = sim.pos != sim.prev_pos;

    let sprite_col = anim.sprite_index % global::PLAYER_ANIMATION_FRAMES;

    // compare: sim state vs animation state => animation actions
    let moving_will_change = next_moving != anim.moving; // TODO: investigate Changed<>
    let facing_will_change = next_facing != anim.facing;
    let will_continue_moving = !moving_will_change && !facing_will_change && next_moving == true;
    if moving_will_change || facing_will_change {
        anim.moving = next_moving;
        anim.facing = next_facing;
        anim.sprite_index =
            anim.facing.row_index_in_player_spritesheet() * global::PLAYER_ANIMATION_FRAMES;
        anim.timer.reset();
        // info!("animate: switch to {}", anim);
    } else if will_continue_moving {
        anim.timer.tick(time.delta());
        anim.sprite_index = anim.facing.row_index_in_player_spritesheet()
            * global::PLAYER_ANIMATION_FRAMES
            + (sprite_col + anim.timer.times_finished_this_tick() as usize)
                % global::PLAYER_ANIMATION_FRAMES;
        // info!("animate: continue moving {}", anim);
    } else {
        // info!("animate: standing still {}", anim);
    };

    // update sprite to new index
    let Some(atlas) = sprite.texture_atlas.as_mut() else {
        return;
    };
    atlas.index = anim.sprite_index;
    // info!("animate: atlas.index={}", atlas.index);
}

#[derive(Component)]
struct Player;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Facing {
    #[default]
    Down,
    Left,
    Up,
    Right,
}

impl Facing {
    const ALL: [Facing; 4] = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];

    fn row_index_in_player_spritesheet(&self) -> usize {
        match self {
            Facing::Down => 0,
            Facing::Left => 1,
            Facing::Up => 2,
            Facing::Right => 3,
        }
    }

    fn key_codes(self) -> &'static [KeyCode] {
        // support both Arrow(s), WASD and HJKL (vim)
        match self {
            Facing::Down => &[KeyCode::ArrowDown, KeyCode::KeyS, KeyCode::KeyJ],
            Facing::Left => &[KeyCode::ArrowLeft, KeyCode::KeyA, KeyCode::KeyH],
            Facing::Up => &[KeyCode::ArrowUp, KeyCode::KeyW, KeyCode::KeyK],
            Facing::Right => &[KeyCode::ArrowRight, KeyCode::KeyD, KeyCode::KeyL],
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

#[derive(Debug, Default, Copy, Clone)]
struct RenderFrameNo(u32); // monotonic counter

impl fmt::Display for RenderFrameNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Component, Debug, Default)]
struct InputBuffer {
    seq_no: RenderFrameNo,
    pressed_down: bool,
    pressed_down_at: RenderFrameNo,
    pressed_left: bool,
    pressed_left_at: RenderFrameNo,
    pressed_up: bool,
    pressed_up_at: RenderFrameNo,
    pressed_right: bool,
    pressed_right_at: RenderFrameNo,
}

impl fmt::Display for InputBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InputBuffer{{seq_no:{}, down:({}, {}), left:({}, {}), up:({}, {}), right:({}, {}),}}",
            self.seq_no,
            self.pressed_down,
            self.pressed_down_at,
            self.pressed_left,
            self.pressed_left_at,
            self.pressed_up,
            self.pressed_up_at,
            self.pressed_right,
            self.pressed_right_at
        )
    }
}

impl InputBuffer {
    fn record(&mut self, key: KeyCode) {
        self.seq_no.0 += 1;
        match key {
            KeyCode::ArrowDown | KeyCode::KeyS | KeyCode::KeyJ => {
                self.pressed_down = true;
                self.pressed_down_at = RenderFrameNo(self.seq_no.0);
            }
            KeyCode::ArrowLeft | KeyCode::KeyA | KeyCode::KeyH => {
                self.pressed_left = true;
                self.pressed_left_at = RenderFrameNo(self.seq_no.0);
            }
            KeyCode::ArrowUp | KeyCode::KeyW | KeyCode::KeyK => {
                self.pressed_up = true;
                self.pressed_up_at = RenderFrameNo(self.seq_no.0);
            }
            KeyCode::ArrowRight | KeyCode::KeyD | KeyCode::KeyL => {
                self.pressed_right = true;
                self.pressed_right_at = RenderFrameNo(self.seq_no.0);
            }
            _ => {}
        }
        // info!("record: {}", self);
    }

    fn clear(&mut self) {
        self.pressed_down = false;
        self.pressed_down_at = RenderFrameNo::default();
        self.pressed_left = false;
        self.pressed_left_at = RenderFrameNo::default();
        self.pressed_up = false;
        self.pressed_up_at = RenderFrameNo::default();
        self.pressed_right = false;
        self.pressed_right_at = RenderFrameNo::default();
    }
}

#[derive(Resource, Debug, Default, Copy, Clone)]
struct SimStepNo(u32);

impl SimStepNo {
    fn new(v: u32) -> Self {
        SimStepNo(v)
    }
}

impl fmt::Display for SimStepNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Component, Debug, Default)]
struct InputCommand {
    sim_step: SimStepNo,
    delta: IVec2, // !IMPORTANT: cheat & flip order: row => y, col => x
    facing: Option<Facing>,
}

impl fmt::Display for InputCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{sim_step:{}, delta=(r{}, c{}), facing={:?} }}",
            self.sim_step, self.delta.y, self.delta.x, self.facing
        )
    }
}


#[derive(Component, Debug)]
struct AnimationState {
    moving: bool,
    timer: Timer,
    facing: Facing,
    sprite_index: usize,
}

impl AnimationState {
    fn default_with_anim_duration(seconds: f32) -> Self {
        return AnimationState {
            moving: false,
            facing: Facing::Down,
            sprite_index: 0 as usize,
            timer: Timer::from_seconds(seconds, TimerMode::Repeating),
        };
    }
}

impl fmt::Display for AnimationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AnimationState{{moving:{}, facing:{}, sprite_index:{}}}",
            self.moving, self.facing, self.sprite_index
        )
    }
}

#[derive(Component, Debug, Default)]
struct SimulationState {
    pos: GridPos,
    prev_pos: GridPos,
    facing: Facing,
    prev_facing: Facing,
}
