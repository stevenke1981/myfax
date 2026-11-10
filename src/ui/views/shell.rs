//! Window shell: nav rail, status bar, banner, toast, confirm dialog (owner: views worker).

use crate::app::{AppState, Msg};
use acme_widgets::WidgetNode;

/// Full window: shell around `page` plus overlay layers.
pub fn frame(_state: &AppState, _page: WidgetNode<Msg>) -> WidgetNode<Msg> {
    todo!("views worker")
}
