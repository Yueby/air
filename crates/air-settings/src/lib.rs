extern crate self as air_settings;

pub mod model;

pub use model::{
    AppLanguage, AppSettings, CloseWindowBehavior, DEFAULT_PROXY_DELAY_TEST_URL,
    GuiThemePreference, WindowSettings,
};
