//! Layout regression checks (test-only). Lays out whole-window views with the
//! same AcmeUI pipeline as the runtime (`views::root` → `to_layout_with_context`
//! → `compute_with_text`) and asserts that nothing leaves the window: no node's
//! right edge passes the window width, and every keyed `scroll.*` region, the
//! toast and the confirm dialog stay inside the window.

use crate::app::state::{ConfirmDialog, Toast, ToastKind};
use crate::app::{AppState, ConfirmAction, Field, Msg, Route};
use crate::config::Settings;
use crate::model::{Contact, DateBucket, Folder, FolderCounts, LogEntry, LogLevel, SentFax, Stats};
use acme_core::NodeId;
use acme_layout::{
    LayoutEngine, LayoutNode, LayoutRect, LayoutSnapshot, Length, WidgetLayoutContext,
};
use acme_text::FontSystem;
use acme_widgets::WidgetNode;
use chrono::{Local, NaiveDate, TimeZone};
use std::path::PathBuf;

/// Tolerance for sub-pixel rounding in the layout engine.
const EPS: f32 = 0.5;
/// Window sizes checked by the regression tests (logical px).
pub const WINDOW_LARGE: (f32, f32) = (1280.0, 800.0);
pub const WINDOW_SMALL: (f32, f32) = (1024.0, 680.0);

/// One laid-out node: depth-first order, optional key and absolute rect.
#[derive(Clone, Debug)]
pub struct Placed {
    pub depth: usize,
    pub key: Option<String>,
    pub rect: LayoutRect,
    /// Inside a keyed `scroll.*` ancestor; its rect is in that region's content space.
    pub in_scroll: bool,
    /// Rect of the parent node and whether the parent is a keyed `scroll.*` region.
    pub parent: Option<(LayoutRect, bool)>,
}

/// Lay out `root` in a `width` × `height` window and return every node's rect.
pub fn place(root: &WidgetNode<Msg>, width: f32, height: f32) -> Vec<Placed> {
    let theme = acme_theme::Theme::light();
    let ctx = WidgetLayoutContext {
        body_font_size: theme.typography.body,
        body_line_height: theme.typography.body * theme.typography.line_height,
        label_font_size: theme.typography.label,
        control_height: theme.control_height(3),
        scale_factor: 1.0,
    };
    let mut tree = root.to_layout_with_context(NodeId::new(1), &ctx);
    if matches!(tree.style.width, Length::Auto) {
        tree.style.width = Length::px(width);
    }
    if matches!(tree.style.height, Length::Auto) {
        tree.style.height = Length::px(height);
    }
    let mut fonts = FontSystem::new();
    let snapshot = LayoutEngine::new()
        .compute_with_text(&tree, (width, height), &mut fonts, 1.0)
        .expect("layout must succeed for a finite window");
    let mut out = Vec::new();
    collect(root, &tree, &snapshot, 0, false, None, &mut out);
    out
}

fn collect(
    widget: &WidgetNode<Msg>,
    layout: &LayoutNode,
    snapshot: &LayoutSnapshot,
    depth: usize,
    in_scroll: bool,
    parent: Option<(LayoutRect, bool)>,
    out: &mut Vec<Placed>,
) {
    assert_eq!(
        widget.children().len(),
        layout.children.len(),
        "widget and layout child counts diverge at depth {depth}"
    );
    let key = widget.key().map(|k| k.as_str().to_string());
    let is_scroll = is_scroll_region(key.as_deref());
    let child_in_scroll = in_scroll || is_scroll;
    if let Some(rect) = snapshot.get(layout.id).copied() {
        out.push(Placed {
            depth,
            key,
            rect,
            in_scroll,
            parent,
        });
        for (child, child_layout) in widget.children().iter().zip(&layout.children) {
            collect(
                child,
                child_layout,
                snapshot,
                depth + 1,
                child_in_scroll,
                Some((rect, is_scroll)),
                out,
            );
        }
        return;
    }
    for (child, child_layout) in widget.children().iter().zip(&layout.children) {
        collect(
            child,
            child_layout,
            snapshot,
            depth + 1,
            child_in_scroll,
            parent,
            out,
        );
    }
}

/// Inbox with a selected fax and a ready page preview at `zoom`.
pub fn preview_ready(zoom: f32) -> AppState {
    use crate::app::state::{PreviewState, PreviewStatus};
    use crate::model::{FaxRecord, FaxSource};
    let mut state = base(Route::Inbox);
    let fax = FaxRecord {
        id: 1,
        received_at: stamp(4),
        sender_number: Some("0223456789".into()),
        sender_name: None,
        pages: 3,
        file_path: PathBuf::from("C:/archive/2026/2026-11/2026-11-04/FAX_0223456789.pdf"),
        file_size: 4096,
        sha256: "a".into(),
        original_name: "FAX_0223456789.pdf".into(),
        source: FaxSource::Manual,
        is_read: true,
        is_starred: false,
        tags: vec![],
        note: String::new(),
        deleted_at: None,
        imported_at: stamp(4),
    };
    state.results.items = vec![fax.clone()];
    state.results.total = 1;
    state.selected = Some(fax);
    state.preview = PreviewState {
        page: 0,
        page_count: 3,
        zoom,
        status: PreviewStatus::Ready,
        image_width: 1000,
        image_height: 1414,
    };
    state
}

/// Largest right edge of any node.
pub fn max_right(placed: &[Placed]) -> f32 {
    placed
        .iter()
        .map(|p| p.rect.x + p.rect.width)
        .fold(0.0, f32::max)
}

/// Every violation of the window-bounds rules, as readable lines.
pub fn violations(placed: &[Placed], width: f32, height: f32) -> Vec<String> {
    let mut out = Vec::new();
    for p in placed {
        let right = p.rect.x + p.rect.width;
        let bottom = p.rect.y + p.rect.height;
        let name = p
            .key
            .clone()
            .unwrap_or_else(|| format!("depth {}", p.depth));
        // Content inside a scroll region may be wider than its viewport (it scrolls);
        // the viewports themselves must fit the window.
        if (!p.in_scroll || is_scroll_region(p.key.as_deref())) && right > width + EPS {
            out.push(format!("{name}: right edge {right:.1} > window {width}"));
        }
        // A nested scroll region is positioned in its parent's content space, so only
        // its own viewport height is bounded; top-level regions must sit in the window.
        let scroll_region = is_scroll_region(p.key.as_deref());
        if scroll_region && p.rect.height > height + EPS {
            out.push(format!(
                "{name}: height {:.1} > window {height}",
                p.rect.height
            ));
        }
        if !p.in_scroll
            && needs_height_check(p.key.as_deref())
            && (bottom > height + EPS || p.rect.y < -EPS)
        {
            out.push(format!(
                "{name}: y {:.1}..{bottom:.1} outside window height {height}",
                p.rect.y
            ));
        }
        // Containment: a child must not spill out of its parent (that would mean the
        // parent's own size is wrong, e.g. a card measured before its text wraps). Inside a
        // scroll region the vertical extent is governed by the region's content height,
        // which AcmeUI derives from the descendant rects, so only horizontal spill is
        // checked there; zoomable page images are exempt (wider than their viewport).
        if let Some((pr, parent_is_scroll)) = p.parent {
            let zoomable = is_zoomable(p.key.as_deref()) && parent_is_scroll;
            let spills_x = !zoomable
                && (p.rect.x < pr.x - EPS || p.rect.x + p.rect.width > pr.x + pr.width + EPS);
            let spills_y = !p.in_scroll
                && (p.rect.y < pr.y - EPS || p.rect.y + p.rect.height > pr.y + pr.height + EPS);
            if spills_x || spills_y {
                out.push(format!(
                    "{name}: {:.0}x{:.0}@({:.0},{:.0}) spills out of parent {:.0}x{:.0}@({:.0},{:.0})",
                    p.rect.width, p.rect.height, p.rect.x, p.rect.y, pr.width, pr.height, pr.x, pr.y
                ));
            }
        }
    }
    out
}

/// Page image slots, which zoom beyond their viewport and scroll.
fn is_zoomable(key: Option<&str>) -> bool {
    matches!(key, Some(k) if k.starts_with("img:"))
}

fn is_scroll_region(key: Option<&str>) -> bool {
    matches!(key, Some(k) if k.starts_with("scroll."))
}

/// Keyed regions that must sit inside the window vertically.
fn needs_height_check(key: Option<&str>) -> bool {
    is_scroll_region(key) || matches!(key, Some("toast" | "modal:dialog" | "shell"))
}

/// Rect of the first node with `key`, if laid out.
pub fn rect_of<'a>(placed: &'a [Placed], key: &str) -> Option<&'a LayoutRect> {
    placed
        .iter()
        .find(|p| p.key.as_deref() == Some(key))
        .map(|p| &p.rect)
}

/// Whole-window view of `state` with the given window size, checked for bounds.
pub fn check_state(state: &AppState, window: (f32, f32)) -> (Vec<Placed>, Vec<String>) {
    let placed = place(&crate::ui::views::root(state), window.0, window.1);
    let problems = violations(&placed, window.0, window.1);
    (placed, problems)
}

fn long_name(seed: usize) -> String {
    format!("EPSON WF-C5890 Series 網路傳真印表機 Network Office East Wing #{seed:02}")
}

fn base(route: Route) -> AppState {
    let mut state = AppState::new(Settings::default());
    state.route = route;
    state
}

fn stamp(day: u32) -> chrono::DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 11, day.clamp(1, 28), 9, 30, 0)
        .unwrap()
}

/// Send page with 20 printers, long names, a long quick-pick list and history.
pub fn send_crowded() -> AppState {
    let mut state = base(Route::Send);
    state.printers = (0..20).map(long_name).collect();
    state.selected_printer = Some(long_name(3));
    state.contacts = (0..30)
        .map(|id| Contact {
            id,
            name: format!("很長很長的聯絡人名稱_{id:03}_ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            fax_number: format!("0223456{id:03}"),
            ..Contact::default()
        })
        .collect();
    state.sent = (0..30)
        .map(|id| SentFax {
            id,
            at: stamp(id as u32),
            file_path: PathBuf::from(format!(
                "C:/Users/demo/Documents/MyFax Archive/very_long_unbroken_file_name_{id:03}_ABCDEFGHIJKLMNOPQRSTUVWXYZ.pdf"
            )),
            printer: long_name(id as usize),
            recipient: "收件人很長很長很長很長很長很長很長很長很長".into(),
            note: "備註ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ".into(),
        })
        .collect();
    state
}

/// Log page with a 300-character unbroken message and a long path.
pub fn log_long() -> AppState {
    let mut state = base(Route::Log);
    let unbroken = "X".repeat(300);
    state.logs = vec![
        LogEntry {
            id: 1,
            at: stamp(2),
            level: LogLevel::Error,
            message: unbroken,
        },
        LogEntry {
            id: 2,
            at: stamp(3),
            level: LogLevel::Info,
            message: format!("已歸檔 C:/very/long/path/{}.pdf", "路徑".repeat(60)),
        },
    ];
    state
}

/// Contacts page with the editor open and long values in every field.
pub fn contacts_editing() -> AppState {
    let mut state = base(Route::Contacts);
    state.contacts = (1..=6)
        .map(|id| Contact {
            id,
            name: format!("聯絡人 {id} ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            fax_number: "0223456789".into(),
            ..Contact::default()
        })
        .collect();
    state.editing_contact = Some(0);
    let long = "Q".repeat(120);
    for field in [
        Field::ContactName,
        Field::ContactCompany,
        Field::ContactFax,
        Field::ContactPhone,
        Field::ContactEmail,
        Field::ContactNote,
    ] {
        state.fields.insert(field, long.clone());
    }
    state
}

/// Settings page with long watch-folder and archive paths.
pub fn settings_long_paths() -> AppState {
    let mut state = base(Route::Settings);
    let folder = PathBuf::from(format!(
        "C:/Users/demo/{}/printer-share",
        "深層資料夾".repeat(20)
    ));
    state.settings.watch_folders = vec![folder.clone(), folder];
    state.settings.archive_root = PathBuf::from(format!("D:/{}", "archive".repeat(30)));
    state
}

/// Settings page with invalid numeric and archive fields (validation captions shown).
pub fn settings_invalid() -> AppState {
    let mut state = base(Route::Settings);
    state.fields.insert(Field::PollInterval, "1".into());
    state.fields.insert(Field::PageSize, "5".into());
    state.fields.insert(Field::ArchiveRoot, "  ".into());
    state
}

/// Stats page over 365 days with long sender names.
pub fn stats_365() -> AppState {
    let mut state = base(Route::Stats);
    state.stats_range = crate::app::StatsRange::Last365Days;
    let start = NaiveDate::from_ymd_opt(2025, 11, 12).unwrap();
    state.stats = Stats {
        total_faxes: 365,
        total_pages: 1200,
        per_day: (0..365)
            .map(|d| DateBucket {
                date: start + chrono::Days::new(d),
                count: d % 7,
            })
            .collect(),
        top_senders: (0..10)
            .map(|i| (format!("ABCDEFGHIJKLMNOPQRSTUVWXYZ傳真號碼{i}"), 10 - i))
            .collect(),
    };
    state
}

/// Long error toast over the inbox.
pub fn toast_long() -> AppState {
    let mut state = base(Route::Inbox);
    state.toast = Some(Toast {
        kind: ToastKind::Error,
        text: "E".repeat(500),
    });
    state
}

/// Confirm dialog over the activity log.
pub fn confirm_clear_log() -> AppState {
    let mut state = base(Route::Log);
    state.confirm = Some(ConfirmDialog {
        title: "清除紀錄".into(),
        body: "確定要清除全部活動紀錄嗎？".into(),
        action: ConfirmAction::ClearLog,
    });
    state
}

/// Inbox in the Trash folder with a populated trash count.
pub fn inbox_trash() -> AppState {
    let mut state = base(Route::Inbox);
    state.query.folder = Folder::Trash;
    state.counts = FolderCounts {
        all: 3,
        unread: 1,
        starred: 0,
        trash: 2,
    };
    state
}

/// Every route at default content, plus the crowded and overlay states.
pub fn all_states() -> Vec<(&'static str, AppState)> {
    let mut out: Vec<(&'static str, AppState)> = Route::ALL
        .iter()
        .map(|&route| (route_name(route), base(route)))
        .collect();
    out.extend([
        ("send_crowded", send_crowded()),
        ("log_long", log_long()),
        ("contacts_editing", contacts_editing()),
        ("settings_long_paths", settings_long_paths()),
        ("settings_invalid", settings_invalid()),
        ("stats_365", stats_365()),
        ("toast_long", toast_long()),
        ("confirm_clear_log", confirm_clear_log()),
        ("inbox_trash", inbox_trash()),
        ("preview_zoom_100", preview_ready(1.0)),
        ("preview_zoom_125", preview_ready(1.25)),
    ]);
    out
}

fn route_name(route: Route) -> &'static str {
    match route {
        Route::Inbox => "inbox",
        Route::Contacts => "contacts",
        Route::Send => "send",
        Route::Stats => "stats",
        Route::Log => "log",
        Route::Settings => "settings",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Debug helper: `cargo test --lib layout_check::tests::dump_state -- --ignored --nocapture`
    /// with `LAYOUT_STATE=<name>` and optional `LAYOUT_DEPTH` prints the rect tree.
    #[test]
    #[ignore = "debug dump; run manually"]
    fn dump_state() {
        let wanted = std::env::var("LAYOUT_STATE").unwrap_or_else(|_| "send".into());
        let max_depth: usize = std::env::var("LAYOUT_DEPTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(12);
        let (_, state) = all_states()
            .into_iter()
            .find(|(name, _)| *name == wanted)
            .expect("unknown LAYOUT_STATE");
        let placed = place(
            &crate::ui::views::root(&state),
            WINDOW_LARGE.0,
            WINDOW_LARGE.1,
        );
        for p in placed.iter().filter(|p| p.depth <= max_depth) {
            eprintln!(
                "{}{} x={:.0} y={:.0} w={:.0} h={:.0}",
                "  ".repeat(p.depth),
                p.key.clone().unwrap_or_else(|| "-".into()),
                p.rect.x,
                p.rect.y,
                p.rect.width,
                p.rect.height
            );
        }
    }

    /// Every state fits the window; prints one summary line per state for the report.
    fn assert_all_fit(window: (f32, f32)) {
        let mut failures = Vec::new();
        for (name, state) in all_states() {
            let (placed, problems) = check_state(&state, window);
            eprintln!(
                "layout {}x{} {name}: max_right={:.0} nodes={}",
                window.0,
                window.1,
                max_right(&placed),
                placed.len()
            );
            if !problems.is_empty() {
                let mut unique = problems.clone();
                unique.dedup();
                unique.sort();
                unique.dedup();
                failures.push(format!(
                    "[{name} @ {}x{}] {} violations, e.g.:\n  {}",
                    window.0,
                    window.1,
                    problems.len(),
                    unique.into_iter().take(6).collect::<Vec<_>>().join("\n  ")
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "layout overflow:\n{}",
            failures.join("\n")
        );
    }

    /// Border width of `scroll.preview`; the image slot must fit inside it.
    const PREVIEW_BORDER: f32 = 1.0;

    #[test]
    fn preview_slot_fits_inside_viewport_at_100_percent() {
        let placed = place(
            &crate::ui::views::root(&preview_ready(1.0)),
            WINDOW_LARGE.0,
            WINDOW_LARGE.1,
        );
        let viewport = rect_of(&placed, "scroll.preview").expect("preview viewport");
        let slot =
            rect_of(&placed, crate::app::controller::PREVIEW_IMAGE_KEY).expect("preview slot");
        let inner = viewport.width - 2.0 * PREVIEW_BORDER;
        assert!(
            slot.width <= inner + EPS,
            "slot {:.1} px wider than viewport content {:.1} px",
            slot.width,
            inner
        );
    }

    #[test]
    fn date_preset_chips_fit_the_filter_column() {
        let placed = place(
            &crate::ui::views::root(&base(Route::Inbox)),
            WINDOW_LARGE.0,
            WINDOW_LARGE.1,
        );
        let filters = rect_of(&placed, "scroll.filters").expect("filter column");
        let content_right = filters.x + filters.width - crate::ui::views::kit::PANE_PAD;
        let chips: Vec<&Placed> = placed
            .iter()
            .filter(|p| {
                p.key
                    .as_deref()
                    .is_some_and(|k| k.starts_with("btn.preset."))
            })
            .collect();
        assert!(chips.len() >= 2, "expected date preset chips");
        for chip in chips {
            let right = chip.rect.x + chip.rect.width;
            assert!(
                right <= content_right + EPS,
                "{:?} right edge {right:.1} > filter content edge {content_right:.1}",
                chip.key
            );
        }
    }

    #[test]
    fn all_states_fit_window_1280x800() {
        assert_all_fit(WINDOW_LARGE);
    }

    #[test]
    fn all_states_fit_window_1024x680() {
        assert_all_fit(WINDOW_SMALL);
    }
}
