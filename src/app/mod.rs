//! Application layer: messages, view-model, controller.

pub mod controller;
pub mod msg;
pub mod platform;
pub mod state;

pub use controller::Controller;
pub use msg::{BoolSetting, ConfirmAction, DatePreset, Field, Msg, Route, StatsRange};
pub use state::AppState;
