use std::fmt;

use bevy::log::info;
use bevy::prelude::{
    App, ButtonInput, Commands, Component, Fixed, IntoScheduleConfigs, KeyCode, Name, Plugin,
    Query, Res, RunFixedMainLoop, RunFixedMainLoopSystems, Sprite, Startup, Time, Timer, TimerMode,
    Transform, Update, With,
};

use bevy::sprite::Anchor;

use crate::global;
use crate::level::GridPos;
use crate::spritesheet::{SpriteSheet, SpriteSheets};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .insert_resource(Time::<Fixed>::from_seconds(
                global::SIMULATION_INTERVAL_SECONDS,
            ))
            .add_systems(
                RunFixedMainLoop,
                collect_input.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            ) // per render frame, but before FixedUpdate (sim) // !IMPORTANT
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

#[derive(Debug, Default)]
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
