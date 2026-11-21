use bevy::prelude::{App, AppExtStates, Plugin, States};

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_state(GameState::Intro);
    }
}

#[derive(States, Debug, Default, Eq, PartialEq, Clone, Hash)]
pub enum GameState {
    #[default]
    Intro,
    Playing,
    GameOver,
}
