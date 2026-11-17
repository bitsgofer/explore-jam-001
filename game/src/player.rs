use bevy::prelude::{App, Commands, Component, Name, Plugin, Res, Startup, Transform};
use bevy::sprite::Anchor;

use crate::global;
use crate::level::GridPos;
use crate::spritesheet::{SpriteSheet, SpriteSheets};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player);
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
    ));
}

#[derive(Component)]
struct Player;
