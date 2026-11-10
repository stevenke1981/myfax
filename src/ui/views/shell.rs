//! Window shell (UI_SPEC §2): nav rail, status bar, new-fax banner, toast and
//! confirm dialog layers around the active page. Owner: views worker (shell).

use crate::app::state::{ConfirmDialog, Toast, ToastKind, WatcherStatus};
use crate::app::{AppState, ConfirmAction, Msg, Route};
use crate::i18n::{Lang, t, tf};
use crate::ui::views::kit::{self, Tone, glyph};
use acme_widgets::style::prelude::*;
use acme_widgets::style::{ColorToken, FontWeight};
use acme_widgets::{
    ButtonSize, ButtonVariant, CardVariant, LabelWrap, WidgetNode, button, card, column,
    label_builder, row, stack,
};
use chrono::{DateTime, Local};

const UNREAD_CAP: u64 = 99;
const ERROR_MAX_UNITS: usize = 60;
const ARCHIVE_MAX_UNITS: usize = 56;
const NAV_ROW_H: f32 = 36.0;
const NAV_BAR_W: f32 = 3.0;
const NAV_GLYPH_W: f32 = 20.0;
const TOAST_MAX_W: f32 = 520.0;
/// Gap between the bottom of the toast and the window bottom: status bar + 8 px.
const TOAST_BOTTOM_GAP: f32 = 36.0;
const DIALOG_W: f32 = 420.0;
const DIALOG_PAD: f32 = 20.0;
const SCRIM_OPACITY: f32 = 0.35;

/// Full window: shell around `page` plus overlay layers. Layers are stacked in
/// z-order (shell < toast < scrim < dialog); overlays exist only when active.
pub fn frame(state: &AppState, page: WidgetNode<Msg>) -> WidgetNode<Msg> {
    let now = Local::now();
    let mut layers = stack::<Msg>()
        .key("root")
        .w(crate::ui::views::kit::full())
        .h(crate::ui::views::kit::full())
        .child(shell(state, page, now));
    if let Some(toast) = &state.toast {
        layers = layers.child(toast_layer(toast));
    }
    if let Some(confirm) = &state.confirm {
        layers = layers
            .child(scrim())
            .child(dialog_layer(state.lang, confirm));
    }
    layers.build()
}

fn shell(state: &AppState, page: WidgetNode<Msg>, now: DateTime<Local>) -> WidgetNode<Msg> {
    column::<Msg>()
        .key("shell")
        .w(crate::ui::views::kit::full())
        .h(crate::ui::views::kit::full())
        .bg(ColorToken::Background)
        .child(body(state, page))
        .child(kit::hdivider())
        .child(status_bar(state, now))
        .build()
}

fn body(state: &AppState, page: WidgetNode<Msg>) -> WidgetNode<Msg> {
    row::<Msg>()
        .key("body")
        .flex_1()
        .child(nav_rail(state))
        .child(kit::vdivider())
        .child(page_area(state, page))
        .build()
}

fn page_area(state: &AppState, page: WidgetNode<Msg>) -> WidgetNode<Msg> {
    let mut area = column::<Msg>()
        .key("pagearea")
        .flex_1()
        .h(crate::ui::views::kit::full());
    if state.new_fax_banner > 0 {
        area = area.child(banner(state));
    }
    area.child(column::<Msg>().flex_1().child(page).build())
        .build()
}

// ── nav rail (§2.1) ─────────────────────────────────────────────────────────

fn nav_rail(state: &AppState) -> WidgetNode<Msg> {
    let mut rail = column::<Msg>()
        .key("nav")
        .w(kit::px(kit::NAV_W))
        .h(crate::ui::views::kit::full())
        .bg(ColorToken::Surface)
        .padding(8.0)
        .gap(4.0)
        .child(brand(state.lang))
        .child(kit::hgap(8.0));
    for route in Route::ALL {
        rail = rail.child(nav_item(state, route));
    }
    rail.child(kit::spacer()).child(watch_mini(state)).build()
}

fn brand(lang: Lang) -> WidgetNode<Msg> {
    column::<Msg>()
        .px(8.0)
        .py(8.0)
        .child(kit::title(t(lang, "shell.app_name")))
        .child(kit::caption(t(lang, "shell.app_tagline")))
        .build()
}

/// Snake-case key segment, nav glyph and localised name of a route.
fn route_meta(lang: Lang, route: Route) -> (&'static str, &'static str, &'static str) {
    match route {
        Route::Inbox => ("inbox", glyph::NAV_INBOX, t(lang, "shell.nav_inbox")),
        Route::Contacts => (
            "contacts",
            glyph::NAV_CONTACTS,
            t(lang, "shell.nav_contacts"),
        ),
        Route::Send => ("send", glyph::NAV_SEND, t(lang, "shell.nav_send")),
        Route::Stats => ("stats", glyph::NAV_STATS, t(lang, "shell.nav_stats")),
        Route::Log => ("log", glyph::NAV_LOG, t(lang, "shell.nav_log")),
        Route::Settings => (
            "settings",
            glyph::NAV_SETTINGS,
            t(lang, "shell.nav_settings"),
        ),
    }
}

fn nav_item(state: &AppState, route: Route) -> WidgetNode<Msg> {
    let lang = state.lang;
    let active = state.route == route;
    let (snake, glyph_text, name) = route_meta(lang, route);
    let text_color = if active {
        ColorToken::AccentForeground
    } else {
        ColorToken::SurfaceForeground
    };
    let bar_color = if active {
        ColorToken::Primary
    } else {
        ColorToken::Surface
    };
    let mut name_label = label_builder::<Msg>(name)
        .font_size(14.0)
        .text_color(text_color)
        .flex_1();
    if active {
        name_label = name_label.font_weight(FontWeight::SemiBold);
    }
    let mut item = row::<Msg>()
        .key(format!("nav.{snake}"))
        .h(kit::px(NAV_ROW_H))
        .px(8.0)
        .gap(8.0)
        .rounded_md();
    if active {
        item = item.bg(ColorToken::Accent);
    }
    item = item
        .child(
            column::<Msg>()
                .w(kit::px(NAV_BAR_W))
                .rounded_full()
                .bg(bar_color)
                .build(),
        )
        .child(
            label_builder::<Msg>(glyph_text)
                .font_size(14.0)
                .text_color(text_color)
                .w(kit::px(NAV_GLYPH_W))
                .build(),
        )
        .child(name_label.build());
    if route == Route::Inbox && state.counts.unread > 0 {
        item = item.child(kit::badge(
            &unread_label(state.counts.unread),
            Tone::Primary,
        ));
    }
    item.on_click(Msg::Navigate(route)).build()
}

/// Unread count for the inbox badge; capped to `99+`.
fn unread_label(count: u64) -> String {
    if count > UNREAD_CAP {
        format!("{UNREAD_CAP}+")
    } else {
        count.to_string()
    }
}

fn watcher_short(lang: Lang, status: &WatcherStatus) -> (&'static str, ColorToken, &'static str) {
    match status {
        WatcherStatus::Running(_) => (
            glyph::DOT,
            ColorToken::Success,
            t(lang, "shell.watch_running_short"),
        ),
        WatcherStatus::Stopped => (
            glyph::RING,
            ColorToken::MutedForeground,
            t(lang, "shell.watch_stopped_short"),
        ),
        WatcherStatus::Error(_) => (
            glyph::DOT,
            ColorToken::Danger,
            t(lang, "shell.watch_error_short"),
        ),
    }
}

fn watch_mini(state: &AppState) -> WidgetNode<Msg> {
    let (dot, color, text) = watcher_short(state.lang, &state.watcher);
    let mut mini = column::<Msg>()
        .key("watchmini")
        .px(8.0)
        .py(8.0)
        .gap(4.0)
        .child(
            row::<Msg>()
                .gap(6.0)
                .child(kit::colored(dot, 12.0, color))
                .child(kit::caption(text))
                .build(),
        );
    if let WatcherStatus::Running(count) = state.watcher {
        mini = mini.child(kit::caption(&count.to_string()));
    }
    mini.build()
}

// ── status bar (§2.2) ───────────────────────────────────────────────────────

fn status_bar(state: &AppState, now: DateTime<Local>) -> WidgetNode<Msg> {
    let lang = state.lang;
    let archive = kit::truncate_middle(
        &state.settings.archive_root.display().to_string(),
        ARCHIVE_MAX_UNITS,
    );
    let archive_text = tf(lang, "shell.archive_path", &[("path", archive.as_str())]);
    let scan_text = last_scan_text(lang, state.last_scan, now);
    row::<Msg>()
        .key("statusbar")
        .h(kit::px(kit::STATUS_H))
        .px(12.0)
        .gap(12.0)
        .bg(ColorToken::Surface)
        .child(watcher_status(state))
        .child(kit::caption("·"))
        .child(kit::caption(&scan_text))
        .child(kit::caption("·"))
        .child(
            label_builder::<Msg>(archive_text)
                .font_size(12.0)
                .text_muted()
                .flex_1()
                .build(),
        )
        .child(
            button("btn.scan_now", t(lang, "shell.scan_now"))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .on_click(Msg::ScanNow),
        )
        .build()
}

fn watcher_status(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let (text, color) = match &state.watcher {
        WatcherStatus::Running(count) => (
            tf(
                lang,
                "shell.status_watching",
                &[("count", count.to_string().as_str())],
            ),
            ColorToken::Success,
        ),
        WatcherStatus::Stopped if state.settings.watch_folders.is_empty() => (
            t(lang, "shell.status_no_folder").to_string(),
            ColorToken::Warning,
        ),
        WatcherStatus::Stopped => (
            t(lang, "shell.status_stopped").to_string(),
            ColorToken::MutedForeground,
        ),
        WatcherStatus::Error(error) => {
            let short = kit::truncate_units(error, ERROR_MAX_UNITS);
            (
                tf(lang, "shell.status_error", &[("error", short.as_str())]),
                ColorToken::Danger,
            )
        }
    };
    kit::colored(&text, 12.0, color)
}

/// `HH:mm:ss` when the last scan happened today, otherwise `YYYY-MM-DD HH:mm:ss`.
fn last_scan_text(lang: Lang, last: Option<DateTime<Local>>, now: DateTime<Local>) -> String {
    match last {
        None => t(lang, "shell.no_scan").to_string(),
        Some(at) => {
            let stamp = if at.date_naive() == now.date_naive() {
                at.format("%H:%M:%S").to_string()
            } else {
                at.format("%Y-%m-%d %H:%M:%S").to_string()
            };
            tf(lang, "shell.last_scan", &[("time", stamp.as_str())])
        }
    }
}

// ── new-fax banner (§2.3, in flow) ──────────────────────────────────────────

fn banner(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let count = state.new_fax_banner.to_string();
    let text = tf(lang, "shell.banner_new_faxes", &[("count", count.as_str())]);
    let mut strip = row::<Msg>()
        .key("banner")
        .h(kit::px(kit::BANNER_H))
        .px(16.0)
        .gap(12.0)
        .bg(ColorToken::InfoSoft)
        .child(kit::colored(glyph::DOT, 14.0, ColorToken::Info))
        .child(
            label_builder::<Msg>(text)
                .font_size(14.0)
                .font_weight(FontWeight::SemiBold)
                .text_color(ColorToken::Info)
                .flex_1()
                .build(),
        );
    if state.route != Route::Inbox {
        strip = strip.child(
            button("btn.banner_view", t(lang, "shell.banner_view"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::XS)
                .on_click(Msg::Navigate(Route::Inbox)),
        );
    }
    strip
        .child(
            button("btn.banner_dismiss", glyph::CLOSE)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::XS)
                .on_click(Msg::DismissBanner),
        )
        .build()
}

// ── toast (§2.4, layer L1) ──────────────────────────────────────────────────

/// Glyph and tone of a toast kind.
fn toast_look(kind: ToastKind) -> (&'static str, Tone) {
    match kind {
        ToastKind::Info => (glyph::INFO, Tone::Info),
        ToastKind::Success => (glyph::OK, Tone::Success),
        ToastKind::Error => (glyph::WARN, Tone::Danger),
    }
}

fn toast_layer(toast: &Toast) -> WidgetNode<Msg> {
    let (glyph_text, tone) = toast_look(toast.kind);
    let (soft, strong) = kit::tone_tokens(tone);
    let bar = row::<Msg>()
        .key("toast")
        .on_click(Msg::DismissToast)
        .px(16.0)
        .py(12.0)
        .gap(12.0)
        .rounded_lg()
        .bg(soft)
        .border(1.0, strong)
        .child(
            label_builder::<Msg>(glyph_text)
                .font_size(14.0)
                .font_weight(FontWeight::SemiBold)
                .text_color(strong)
                .build(),
        )
        .child(
            label_builder::<Msg>(toast.text.as_str())
                .font_size(14.0)
                .wrap(LabelWrap::Word)
                .text_color(ColorToken::Foreground)
                .flex_1()
                .build(),
        )
        .child(kit::colored(
            glyph::CLOSE,
            14.0,
            ColorToken::MutedForeground,
        ))
        .build();
    let toast_card = card::<Msg>()
        .variant(CardVariant::Elevated)
        .max_w(kit::px(TOAST_MAX_W))
        .child(bar)
        .build();
    column::<Msg>()
        .key("toast_layer")
        .w(crate::ui::views::kit::full())
        .h(crate::ui::views::kit::full())
        .child(kit::spacer())
        .child(
            row::<Msg>()
                .w(crate::ui::views::kit::full())
                .child(kit::spacer())
                .child(toast_card)
                .child(kit::spacer())
                .build(),
        )
        .child(kit::hgap(TOAST_BOTTOM_GAP))
        .build()
}

// ── confirm dialog (§2.5, layers L2 + L3) ───────────────────────────────────

/// Full-window hit-blocking scrim (`modal:` key swallows pointer input).
fn scrim() -> WidgetNode<Msg> {
    column::<Msg>()
        .key("modal:scrim")
        .w(crate::ui::views::kit::full())
        .h(crate::ui::views::kit::full())
        .bg(ColorToken::Foreground)
        .opacity(SCRIM_OPACITY)
        .build()
}

fn dialog_layer(lang: Lang, confirm: &ConfirmDialog) -> WidgetNode<Msg> {
    let dialog = card::<Msg>()
        .key("modal:dialog")
        .variant(CardVariant::Elevated)
        .w(kit::px(DIALOG_W))
        .padding(DIALOG_PAD)
        .gap(16.0)
        .bg(ColorToken::SurfaceOverlay)
        .child(kit::section(confirm.title.as_str()))
        .child(kit::body_wrap(confirm.body.as_str()))
        .child(confirm_buttons(lang, &confirm.action))
        .build();
    column::<Msg>()
        .key("modal:dialog_layer")
        .w(crate::ui::views::kit::full())
        .h(crate::ui::views::kit::full())
        .child(kit::spacer())
        .child(
            row::<Msg>()
                .w(crate::ui::views::kit::full())
                .child(kit::spacer())
                .child(dialog)
                .child(kit::spacer())
                .build(),
        )
        .child(kit::spacer())
        .build()
}

/// Cancel (first, Secondary) and the destructive confirm button (Danger).
fn confirm_buttons(lang: Lang, action: &ConfirmAction) -> WidgetNode<Msg> {
    let yes_label = match action {
        ConfirmAction::DeletePermanently(_) | ConfirmAction::DeleteContact(_) => {
            t(lang, "shell.confirm_delete")
        }
        ConfirmAction::EmptyTrash => t(lang, "shell.confirm_empty"),
        ConfirmAction::ClearLog => t(lang, "shell.confirm_clear"),
    };
    row::<Msg>()
        .key("dialog.buttons")
        .gap(8.0)
        .child(kit::spacer())
        .child(
            button("btn.confirm_no", t(lang, "common.cancel"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Medium)
                .on_click(Msg::ConfirmNo),
        )
        .child(
            button("btn.confirm_yes", yes_label)
                .variant(ButtonVariant::Danger)
                .size(ButtonSize::Medium)
                .on_click(Msg::ConfirmYes),
        )
        .build()
}

// ── test helpers shared by the page tests ───────────────────────────────────

#[cfg(test)]
pub(crate) mod test_util {
    use crate::app::Msg;
    use acme_widgets::WidgetNode;

    /// Depth-first search for the first node carrying `key`.
    pub fn find_key<'a>(node: &'a WidgetNode<Msg>, key: &str) -> Option<&'a WidgetNode<Msg>> {
        if node.key().is_some_and(|k| k.as_str() == key) {
            return Some(node);
        }
        node.children()
            .iter()
            .find_map(|child| find_key(child, key))
    }

    /// Every widget key in the tree, depth-first.
    pub fn keys(node: &WidgetNode<Msg>) -> Vec<String> {
        let mut out: Vec<String> = node
            .key()
            .map(|k| k.as_str().to_string())
            .into_iter()
            .collect();
        for child in node.children() {
            out.extend(keys(child));
        }
        out
    }

    /// Every label text in the tree, depth-first.
    pub fn texts(node: &WidgetNode<Msg>) -> Vec<String> {
        let mut out = Vec::new();
        if let WidgetNode::Label(label) = node {
            out.push(label.text.clone());
        }
        for child in node.children() {
            out.extend(texts(child));
        }
        out
    }

    /// Messages a click or activation can reach: container `on_click` and enabled buttons.
    pub fn messages(node: &WidgetNode<Msg>) -> Vec<Msg> {
        let mut out = Vec::new();
        match node {
            WidgetNode::Row(c) | WidgetNode::Column(c) | WidgetNode::Stack(c) => {
                out.extend(c.message.clone());
            }
            WidgetNode::Button(b) => out.extend(b.activate().cloned()),
            _ => {}
        }
        for child in node.children() {
            out.extend(messages(child));
        }
        out
    }

    /// The button with `key`, if present in the tree.
    pub fn button_at<'a>(
        node: &'a WidgetNode<Msg>,
        key: &str,
    ) -> Option<&'a acme_widgets::Button<Msg>> {
        match find_key(node, key)? {
            WidgetNode::Button(button) => Some(button),
            _ => None,
        }
    }

    /// `invalid` flag of the text input with `key`, if present.
    pub fn input_invalid(node: &WidgetNode<Msg>, key: &str) -> Option<bool> {
        match find_key(node, key)? {
            WidgetNode::TextInput(input) => Some(input.invalid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
