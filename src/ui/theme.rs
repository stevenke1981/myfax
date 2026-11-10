//! Application theme selection (owner: views worker).

use crate::config::ThemeChoice;
use acme_theme::Theme;

/// Theme for the chosen appearance.
pub fn app_theme(choice: ThemeChoice) -> Theme {
    match choice {
        ThemeChoice::Light => Theme::light(),
        ThemeChoice::Dark => Theme::dark(),
    }
}
