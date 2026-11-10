//! Application screens. Pure functions of [`AppState`] → `WidgetNode<Msg>`.
//! Owner: views workers (one file each). `root` composes them.

use crate::app::{AppState, Msg};
use acme_widgets::WidgetNode;

/// Whole-window view.
pub fn root(_state: &AppState) -> WidgetNode<Msg> {
    todo!("views worker")
}
