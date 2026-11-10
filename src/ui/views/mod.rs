//! Application screens. Pure functions of [`AppState`] → `WidgetNode<Msg>`.
//! Spec: docs/team/myfax/UI_SPEC.md. One owner per file.

pub mod contacts;
pub mod detail;
pub mod inbox;
pub mod kit;
pub mod log;
pub mod send;
pub mod settings;
pub mod shell;
pub mod stats;

use crate::app::{AppState, Msg, Route};
use acme_widgets::WidgetNode;

/// Whole-window view: the shell around the active page.
pub fn root(state: &AppState) -> WidgetNode<Msg> {
    let page = match state.route {
        Route::Inbox => inbox::view(state),
        Route::Contacts => contacts::view(state),
        Route::Send => send::view(state),
        Route::Stats => stats::view(state),
        Route::Log => log::view(state),
        Route::Settings => settings::view(state),
    };
    shell::frame(state, page)
}
