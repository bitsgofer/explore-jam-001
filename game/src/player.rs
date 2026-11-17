use std::fmt;

use bevy::log::info;
use bevy::prelude::{
    App, Commands, Component, Fixed, Name, Plugin, Query, Res, Sprite, Startup, Time, Timer,
    TimerMode, Transform, Update, With,
};
use bevy::sprite::Anchor;

use crate::global;
use crate::level::GridPos;
use crate::spritesheet::{SpriteSheet, SpriteSheets};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
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
