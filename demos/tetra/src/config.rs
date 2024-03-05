use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub assets: String,
    #[serde(default)]
    pub scripting: ScriptingConfig,
    #[serde(default)]
    pub game: GameConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptingConfig {
    #[serde(default = "ScriptingConfig::default_entry")]
    pub entry: String,
    #[serde(default = "ScriptingConfig::default_stack_capacity")]
    pub stack_capacity: usize,
    #[serde(default = "ScriptingConfig::default_registers_capacity")]
    pub registers_capacity: usize,
}

impl Default for ScriptingConfig {
    fn default() -> Self {
        Self {
            entry: Self::default_entry(),
            stack_capacity: Self::default_stack_capacity(),
            registers_capacity: Self::default_registers_capacity(),
        }
    }
}

impl ScriptingConfig {
    fn default_entry() -> String {
        "game.simp".to_owned()
    }

    fn default_stack_capacity() -> usize {
        10240
    }

    fn default_registers_capacity() -> usize {
        10240
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(default = "GameConfig::default_title")]
    pub title: String,
    #[serde(default = "GameConfig::default_window_width")]
    pub window_width: usize,
    #[serde(default = "GameConfig::default_window_height")]
    pub window_height: usize,
    #[serde(default = "GameConfig::default_fullscreen")]
    pub fullscreen: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            title: Self::default_title(),
            window_width: Self::default_window_width(),
            window_height: Self::default_window_height(),
            fullscreen: Self::default_fullscreen(),
        }
    }
}

impl GameConfig {
    fn default_title() -> String {
        "Game".to_owned()
    }

    fn default_window_width() -> usize {
        1024
    }

    fn default_window_height() -> usize {
        720
    }

    fn default_fullscreen() -> bool {
        false
    }
}
