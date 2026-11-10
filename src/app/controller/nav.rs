//! Navigation and native pickers for path fields.

use super::Controller;
use crate::app::msg::{Field, Route};
use std::path::PathBuf;

const DOCUMENT_EXTENSIONS: &[&str] = &["pdf", "tif", "tiff"];
const CSV_EXTENSIONS: &[&str] = &["csv"];

impl Controller {
    /// Switch page. Entering a page refreshes the data it shows; Inbox clears the banner.
    pub(super) fn navigate(&mut self, route: Route) {
        self.state.route = route;
        match route {
            Route::Inbox => self.state.new_fax_banner = 0,
            Route::Contacts => self.refresh_contacts(),
            Route::Send => {
                if self.state.printers.is_empty() {
                    self.refresh_printers(false);
                }
            }
            Route::Stats => self.refresh_stats(),
            Route::Log => self.refresh_logs(),
            Route::Settings => {}
        }
    }

    /// Open the native picker for a path field and store the result in it.
    pub(super) fn browse(&mut self, field: Field) {
        if !field.is_path() {
            return;
        }
        let start = self.field_path(field);
        let picked = if field.picks_folder() {
            self.platform.pick_folder(start.as_deref())
        } else {
            let extensions = if field == Field::CsvPath {
                CSV_EXTENSIONS
            } else {
                DOCUMENT_EXTENSIONS
            };
            self.platform.pick_file(start.as_deref(), extensions)
        };
        if let Some(path) = picked {
            self.state.fields.insert(field, path.display().to_string());
        }
    }

    /// Current value of a path field as a starting directory for pickers.
    fn field_path(&self, field: Field) -> Option<PathBuf> {
        let text = self.state.field(field).trim();
        (!text.is_empty()).then(|| PathBuf::from(text))
    }
}
