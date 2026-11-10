//! Settings page (UI_SPEC §8): language, theme, receiving, storage and the
//! Epson setup help card. Owner: views worker (settings).

use crate::app::{AppState, BoolSetting, Field, Msg};
use crate::config::ThemeChoice;
use crate::i18n::{Lang, t};
use crate::ui::views::kit::{self, glyph};
use acme_widgets::style::ColorToken;
use acme_widgets::style::prelude::*;
use acme_widgets::{
    ButtonSize, ButtonVariant, CardVariant, LabelWrap, TextInput, WidgetNode, button, card, column,
    label_builder, row, text_input,
};
use std::path::Path;

const CONTENT_MAX_W: f32 = 720.0;
const WATCH_ROW_H: f32 = 32.0;
const WATCH_PATH_MAX_UNITS: usize = 64;
const FOOTER_H: f32 = 56.0;
const POLL_MIN: u64 = 2;
const POLL_MAX: u64 = 3600;
const TRASH_MIN: u64 = 0;
const TRASH_MAX: u64 = 3650;
const PAGE_MIN: u64 = 10;
const PAGE_MAX: u64 = 500;

/// Whole page body (right of the nav rail).
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    column::<Msg>()
        .key("page.settings")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(kit::page_header(
            t(state.lang, "settings.title"),
            None,
            Vec::new(),
        ))
        .child(scroll_region(state))
        .child(kit::hdivider())
        .child(footer(state))
        .build()
}

fn scroll_region(state: &AppState) -> WidgetNode<Msg> {
    let content = column::<Msg>()
        .max_w(kit::px(CONTENT_MAX_W))
        .gap(16.0)
        .child(general_card(state))
        .child(receive_card(state))
        .child(storage_card(state))
        .child(help_card(state.lang))
        .build();
    column::<Msg>()
        .key("scroll.settings")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .overflow_scroll()
        .padding(24.0)
        .child(content)
        .build()
}

fn choice_variant(on: bool) -> ButtonVariant {
    if on {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    }
}

fn general_card(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut languages = row::<Msg>().gap(8.0);
    for choice in Lang::ALL {
        languages = languages.child(
            button(format!("btn.lang.{}", choice.code()), choice.native_name())
                .variant(choice_variant(state.lang == choice))
                .size(ButtonSize::Small)
                .on_click(Msg::SetLanguage(choice)),
        );
    }
    let themes = row::<Msg>()
        .gap(8.0)
        .child(
            button("btn.theme.light", t(lang, "settings.theme_light"))
                .variant(choice_variant(state.theme == ThemeChoice::Light))
                .size(ButtonSize::Small)
                .on_click(Msg::SetTheme(ThemeChoice::Light)),
        )
        .child(
            button("btn.theme.dark", t(lang, "settings.theme_dark"))
                .variant(choice_variant(state.theme == ThemeChoice::Dark))
                .size(ButtonSize::Small)
                .on_click(Msg::SetTheme(ThemeChoice::Dark)),
        );
    card::<Msg>()
        .key("set.general")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(12.0)
        .w(crate::ui::views::kit::full())
        .child(kit::section(t(lang, "settings.section_general")))
        .child(kit::caption(t(lang, "settings.language")))
        .child(languages.build())
        .child(kit::caption(t(lang, "settings.theme")))
        .child(themes.build())
        .build()
}

// ── receiving ───────────────────────────────────────────────────────────────

fn receive_card(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let folders = &state.settings.watch_folders;
    let mut receive = card::<Msg>()
        .key("set.receive")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(12.0)
        .w(crate::ui::views::kit::full())
        .child(kit::section(t(lang, "settings.section_receive")))
        .child(kit::caption(t(lang, "settings.watch_folders")));
    if folders.is_empty() {
        receive = receive.child(kit::caption(t(lang, "settings.watch_empty")));
    }
    for (index, folder) in folders.iter().enumerate() {
        receive = receive.child(watch_row(lang, index, folder));
    }
    receive = receive.child(watch_add_row(state)).child(poll_field(state));
    for node in toggles(state) {
        receive = receive.child(node);
    }
    receive.child(forward_field(state)).build()
}

fn watch_row(lang: Lang, index: usize, folder: &Path) -> WidgetNode<Msg> {
    let shown = kit::truncate_middle(&folder.display().to_string(), WATCH_PATH_MAX_UNITS);
    row::<Msg>()
        .key(format!("watch.{index}"))
        .h(kit::px(WATCH_ROW_H))
        .gap(8.0)
        .child(flex_label(&shown))
        .child(
            button(
                format!("btn.watch_remove.{index}"),
                t(lang, "common.remove"),
            )
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::XS)
            .on_click(Msg::RemoveWatchFolder(index)),
        )
        .build()
}

fn watch_add_row(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    row::<Msg>()
        .gap(8.0)
        .child(
            input(state, Field::WatchFolderInput)
                .placeholder(t(lang, "settings.watch_ph"))
                .flex_1()
                .flex_shrink(1.0)
                .min_h(crate::ui::views::kit::px(0.0))
                .build(),
        )
        .child(browse(
            lang,
            "btn.browse.watch_folder",
            Field::WatchFolderInput,
        ))
        .child(
            button("btn.watch_add", t(lang, "common.add"))
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Small)
                .on_click(Msg::AddWatchFolder),
        )
        .build()
}

fn poll_field(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let valid = is_int_in(state.field(Field::PollInterval), POLL_MIN, POLL_MAX);
    number_field(
        state,
        Field::PollInterval,
        t(lang, "settings.poll_interval"),
        t(lang, "settings.poll_hint"),
        valid,
        t(lang, "settings.err_poll"),
    )
}

fn forward_field(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let line = row::<Msg>()
        .gap(8.0)
        .child(
            input(state, Field::ForwardFolder)
                .placeholder(t(lang, "settings.forward_ph"))
                .flex_1()
                .flex_shrink(1.0)
                .min_h(crate::ui::views::kit::px(0.0))
                .build(),
        )
        .child(browse(
            lang,
            "btn.browse.forward_folder",
            Field::ForwardFolder,
        ))
        .build();
    kit::field(t(lang, "settings.forward_folder"), line)
}

/// The four switches. Each applies immediately (`ToggleSetting`).
fn toggles(state: &AppState) -> Vec<WidgetNode<Msg>> {
    let lang = state.lang;
    let settings = &state.settings;
    [
        toggle(
            BoolSetting::DeleteSourceAfterImport,
            settings.delete_source_after_import,
            "delete_source",
            t(lang, "settings.opt_delete_source"),
        ),
        toggle(
            BoolSetting::AcceptTiff,
            settings.accept_tiff,
            "accept_tiff",
            t(lang, "settings.opt_accept_tiff"),
        ),
        toggle(
            BoolSetting::AutoPrint,
            settings.auto_print,
            "auto_print",
            t(lang, "settings.opt_auto_print"),
        ),
        toggle(
            BoolSetting::NotifyNewFax,
            settings.notify_new_fax,
            "notify",
            t(lang, "settings.opt_notify"),
        ),
    ]
    .into_iter()
    .collect()
}

fn toggle(setting: BoolSetting, on: bool, key: &str, text: &str) -> WidgetNode<Msg> {
    let mark = if on {
        glyph::CHECK_ON
    } else {
        glyph::CHECK_OFF
    };
    row::<Msg>()
        .child(
            button(format!("btn.toggle.{key}"), format!("{mark} {text}"))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Small)
                .on_click(Msg::ToggleSetting(setting)),
        )
        .build()
}

// ── storage ─────────────────────────────────────────────────────────────────

fn storage_card(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let archive_valid = !state.field(Field::ArchiveRoot).trim().is_empty();
    let mut archive = input(state, Field::ArchiveRoot);
    if !archive_valid {
        archive = archive
            .invalid(true)
            .validation(t(lang, "settings.err_archive"));
    }
    let archive_line = row::<Msg>()
        .gap(8.0)
        .child(
            archive
                .flex_1()
                .flex_shrink(1.0)
                .min_h(crate::ui::views::kit::px(0.0))
                .build(),
        )
        .child(browse(lang, "btn.browse.archive_root", Field::ArchiveRoot))
        .child(
            button("btn.open_archive", t(lang, "settings.open_archive"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Small)
                .on_click(Msg::OpenArchiveFolder),
        )
        .build();
    let archive_field = column::<Msg>()
        .gap(4.0)
        .child(kit::field(t(lang, "settings.archive_root"), archive_line))
        .child(kit::caption(t(lang, "settings.archive_hint")))
        .build();
    let retention = state.field(Field::TrashRetention);
    let page = state.field(Field::PageSize);
    card::<Msg>()
        .key("set.storage")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(12.0)
        .w(crate::ui::views::kit::full())
        .child(kit::section(t(lang, "settings.section_storage")))
        .child(archive_field)
        .child(number_field(
            state,
            Field::TrashRetention,
            t(lang, "settings.trash_retention"),
            t(lang, "settings.trash_hint"),
            is_int_in(retention, TRASH_MIN, TRASH_MAX),
            t(lang, "settings.err_trash"),
        ))
        .child(number_field(
            state,
            Field::PageSize,
            t(lang, "settings.page_size"),
            t(lang, "settings.page_hint"),
            is_int_in(page, PAGE_MIN, PAGE_MAX),
            t(lang, "settings.err_page"),
        ))
        .build()
}

fn help_card(lang: Lang) -> WidgetNode<Msg> {
    card::<Msg>()
        .key("set.help")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(8.0)
        .w(crate::ui::views::kit::full())
        .bg(ColorToken::InfoSoft)
        .child(kit::section(t(lang, "settings.help_title")))
        .child(kit::caption_wrap(t(lang, "settings.help_intro")))
        .child(kit::body_wrap(t(lang, "settings.help_step1")))
        .child(kit::body_wrap(t(lang, "settings.help_step2")))
        .child(kit::body_wrap(t(lang, "settings.help_step3")))
        .child(kit::body_wrap(t(lang, "settings.help_step4")))
        .child(kit::caption_wrap(t(lang, "settings.help_note")))
        .build()
}

// ── footer ──────────────────────────────────────────────────────────────────

fn footer(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let mut bar = row::<Msg>()
        .key("settingsfooter")
        .h(kit::px(FOOTER_H))
        .px(24.0)
        .gap(12.0)
        .bg(ColorToken::Surface)
        .child(
            button("btn.save_settings", t(lang, "settings.save_btn"))
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Medium)
                .on_click(Msg::SaveSettings),
        );
    if is_dirty(state) {
        bar = bar.child(kit::colored(
            t(lang, "settings.unsaved"),
            12.0,
            ColorToken::Warning,
        ));
    }
    bar.child(
        label_builder::<Msg>(t(lang, "settings.apply_note"))
            .font_size(12.0)
            .text_muted()
            .wrap(LabelWrap::Word)
            .flex_1()
            .flex_shrink(1.0)
            .build(),
    )
    .build()
}

// ── shared helpers ──────────────────────────────────────────────────────────

/// Text input bound to `field` (typing arrives as `TextChanged`, no message).
fn input(state: &AppState, field: Field) -> TextInput<Msg> {
    text_input::<Msg>(field.key()).value(state.field(field))
}

/// "Browse…" button that asks the controller to open a native picker for `field`.
fn browse(lang: Lang, key: &str, field: Field) -> WidgetNode<Msg> {
    button(key, t(lang, "common.browse"))
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Small)
        .on_click(Msg::Browse(field))
}

/// Flexible single-line label (no wrap, fills the row).
fn flex_label(text: &str) -> WidgetNode<Msg> {
    label_builder::<Msg>(text)
        .font_size(14.0)
        .flex_1()
        .flex_shrink(1.0)
        .build()
}

/// Labelled integer field with a caption hint and inline validation.
fn number_field(
    state: &AppState,
    field: Field,
    label: &str,
    hint: &str,
    valid: bool,
    error: &str,
) -> WidgetNode<Msg> {
    let mut text = input(state, field);
    if !valid {
        text = text.invalid(true).validation(error);
    }
    column::<Msg>()
        .gap(4.0)
        .child(kit::field(label, text.build()))
        .child(kit::caption(hint))
        .build()
}

/// True when `raw` parses as an integer in `min..=max`.
fn is_int_in(raw: &str, min: u64, max: u64) -> bool {
    raw.trim()
        .parse::<u64>()
        .is_ok_and(|value| (min..=max).contains(&value))
}

/// True when the persisted text fields differ from the edited ones, or a watch
/// folder was typed but not added yet.
fn is_dirty(state: &AppState) -> bool {
    let s = &state.settings;
    let forward = s
        .forward_copy_folder
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    state.field(Field::ArchiveRoot) != s.archive_root.display().to_string()
        || state.field(Field::PollInterval) != s.poll_interval_secs.to_string()
        || state.field(Field::TrashRetention) != s.trash_retention_days.to_string()
        || state.field(Field::PageSize) != s.page_size.to_string()
        || state.field(Field::ForwardFolder) != forward
        || !state.field(Field::WatchFolderInput).trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::ui::views::shell::test_util::{
        button_at, find_key, input_invalid, keys, messages, texts,
    };
    use acme_widgets::style::prelude::Overflow;
    use std::path::PathBuf;

    fn state() -> AppState {
        AppState::new(Settings::default())
    }

    /// State as the controller leaves it after priming: fields mirror settings.
    fn primed() -> AppState {
        let settings = Settings {
            watch_folders: vec![PathBuf::from("C:/watch/a"), PathBuf::from("C:/watch/b")],
            ..Settings::default()
        };
        let mut st = AppState::new(settings);
        let archive = st.settings.archive_root.display().to_string();
        st.fields.insert(Field::ArchiveRoot, archive);
        st.fields.insert(Field::PollInterval, "5".into());
        st.fields.insert(Field::TrashRetention, "30".into());
        st.fields.insert(Field::PageSize, "50".into());
        st.fields.insert(Field::ForwardFolder, String::new());
        st
    }

    fn view_of(st: &AppState) -> WidgetNode<Msg> {
        view(st)
    }

    #[test]
    fn page_has_scroll_region_footer_and_all_sections() {
        let root = view_of(&primed());
        for key in [
            "page.settings",
            "scroll.settings",
            "set.general",
            "set.receive",
            "set.storage",
            "set.help",
            "settingsfooter",
            "btn.save_settings",
        ] {
            assert!(find_key(&root, key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn scroll_region_uses_overflow_scroll() {
        let root = view_of(&primed());
        match find_key(&root, "scroll.settings") {
            Some(WidgetNode::Column(c)) => {
                assert_eq!(c.style.overflow, Some(Overflow::Scroll));
            }
            other => panic!("scroll region is not a column: {other:?}"),
        }
    }

    #[test]
    fn language_buttons_emit_set_language_and_mark_current() {
        let root = view_of(&primed());
        let zh = button_at(&root, "btn.lang.zh-TW").expect("zh button");
        let en = button_at(&root, "btn.lang.en").expect("en button");
        assert_eq!(zh.variant, ButtonVariant::Primary);
        assert_eq!(en.variant, ButtonVariant::Secondary);
        assert_eq!(en.label, "English");
        let msgs = messages(&root);
        assert!(msgs.contains(&Msg::SetLanguage(Lang::En)));
        assert!(msgs.contains(&Msg::SetLanguage(Lang::ZhTw)));
    }

    #[test]
    fn theme_buttons_follow_state_theme() {
        let mut st = primed();
        st.theme = ThemeChoice::Dark;
        let root = view_of(&st);
        assert_eq!(
            button_at(&root, "btn.theme.dark").expect("dark").variant,
            ButtonVariant::Primary
        );
        assert_eq!(
            button_at(&root, "btn.theme.light").expect("light").variant,
            ButtonVariant::Secondary
        );
        let msgs = messages(&root);
        assert!(msgs.contains(&Msg::SetTheme(ThemeChoice::Light)));
        assert!(msgs.contains(&Msg::SetTheme(ThemeChoice::Dark)));
    }

    #[test]
    fn watch_rows_remove_by_index_for_two_folders() {
        let root = view_of(&primed());
        assert!(find_key(&root, "watch.0").is_some());
        assert!(find_key(&root, "watch.1").is_some());
        assert!(find_key(&root, "watch.2").is_none());
        assert!(messages(&root).contains(&Msg::RemoveWatchFolder(1)));
        assert!(texts(&root).contains(&"C:/watch/b".to_string()));
    }

    #[test]
    fn watch_empty_hint_only_without_folders() {
        let empty = view_of(&state());
        let empty_hint = t(Lang::ZhTw, "settings.watch_empty").to_string();
        assert!(texts(&empty).contains(&empty_hint));
        let filled = view_of(&primed());
        assert!(!texts(&filled).contains(&empty_hint));
    }

    #[test]
    fn add_browse_and_save_actions_are_wired() {
        let root = view_of(&primed());
        let msgs = messages(&root);
        assert!(msgs.contains(&Msg::AddWatchFolder));
        assert!(msgs.contains(&Msg::Browse(Field::WatchFolderInput)));
        assert!(msgs.contains(&Msg::Browse(Field::ArchiveRoot)));
        assert!(msgs.contains(&Msg::Browse(Field::ForwardFolder)));
        assert!(msgs.contains(&Msg::OpenArchiveFolder));
        assert!(msgs.contains(&Msg::SaveSettings));
    }

    #[test]
    fn toggles_show_check_marks_and_emit_toggle_setting() {
        let mut st = primed();
        st.settings.auto_print = true;
        let root = view_of(&st);
        let auto = button_at(&root, "btn.toggle.auto_print").expect("auto_print");
        assert!(auto.label.starts_with(glyph::CHECK_ON));
        let delete = button_at(&root, "btn.toggle.delete_source").expect("delete_source");
        assert!(delete.label.starts_with(glyph::CHECK_OFF));
        let msgs = messages(&root);
        assert!(msgs.contains(&Msg::ToggleSetting(BoolSetting::AutoPrint)));
        assert!(msgs.contains(&Msg::ToggleSetting(BoolSetting::DeleteSourceAfterImport)));
        assert!(msgs.contains(&Msg::ToggleSetting(BoolSetting::AcceptTiff)));
        assert!(msgs.contains(&Msg::ToggleSetting(BoolSetting::NotifyNewFax)));
    }

    #[test]
    fn numeric_fields_flag_out_of_range_values() {
        let mut st = primed();
        st.fields.insert(Field::PollInterval, "1".into());
        st.fields.insert(Field::PageSize, "5".into());
        st.fields.insert(Field::TrashRetention, "3651".into());
        st.fields.insert(Field::ArchiveRoot, "  ".into());
        let root = view_of(&st);
        assert_eq!(input_invalid(&root, "field.poll_interval"), Some(true));
        assert_eq!(input_invalid(&root, "field.page_size"), Some(true));
        assert_eq!(input_invalid(&root, "field.trash_retention"), Some(true));
        assert_eq!(input_invalid(&root, "field.archive_root"), Some(true));
        assert!(texts(&root).contains(&t(Lang::ZhTw, "settings.poll_hint").to_string()));
    }

    #[test]
    fn valid_numeric_fields_are_not_flagged() {
        let root = view_of(&primed());
        assert_eq!(input_invalid(&root, "field.poll_interval"), Some(false));
        assert_eq!(input_invalid(&root, "field.page_size"), Some(false));
        assert_eq!(input_invalid(&root, "field.trash_retention"), Some(false));
        assert_eq!(input_invalid(&root, "field.archive_root"), Some(false));
    }

    #[test]
    fn text_fields_use_their_field_keys() {
        let root = view_of(&primed());
        let all = keys(&root);
        for field in [
            Field::ArchiveRoot,
            Field::PollInterval,
            Field::TrashRetention,
            Field::PageSize,
            Field::ForwardFolder,
            Field::WatchFolderInput,
        ] {
            assert!(all.contains(&field.key().to_string()), "{}", field.key());
        }
    }

    #[test]
    fn unsaved_caption_only_when_dirty() {
        let clean = view_of(&primed());
        let unsaved = t(Lang::ZhTw, "settings.unsaved").to_string();
        assert!(!texts(&clean).contains(&unsaved));

        let mut poll = primed();
        poll.fields.insert(Field::PollInterval, "10".into());
        assert!(texts(&view_of(&poll)).contains(&unsaved));

        let mut typed = primed();
        typed
            .fields
            .insert(Field::WatchFolderInput, "D:/new".into());
        assert!(texts(&view_of(&typed)).contains(&unsaved));
    }

    #[test]
    fn help_card_lists_steps_and_keeps_unc_backslashes() {
        let root = view_of(&primed());
        assert!(find_key(&root, "set.help").is_some());
        let labels = texts(&root);
        let step2 = t(Lang::ZhTw, "settings.help_step2");
        assert!(step2.contains("\\\\PC-NAME\\FaxInbox"));
        assert!(labels.contains(&step2.to_string()));
        assert!(labels.contains(&t(Lang::ZhTw, "settings.help_step4").to_string()));
    }

    #[test]
    fn is_int_in_respects_bounds() {
        assert!(!is_int_in("1", POLL_MIN, POLL_MAX));
        assert!(is_int_in("2", POLL_MIN, POLL_MAX));
        assert!(is_int_in(" 3600 ", POLL_MIN, POLL_MAX));
        assert!(!is_int_in("3601", POLL_MIN, POLL_MAX));
        assert!(!is_int_in("abc", POLL_MIN, POLL_MAX));
        assert!(is_int_in("0", TRASH_MIN, TRASH_MAX));
        assert!(!is_int_in("-1", TRASH_MIN, TRASH_MAX));
    }

    #[test]
    fn is_dirty_compares_against_settings() {
        assert!(!is_dirty(&primed()));
        assert!(is_dirty(&state()), "unprimed fields differ from settings");
        let mut st = primed();
        st.settings.forward_copy_folder = Some(PathBuf::from("E:/copy"));
        assert!(is_dirty(&st));
    }
}
