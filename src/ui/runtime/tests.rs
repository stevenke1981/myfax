//! Headless tests for the UI runtime: no window, no GPU, no real filesystem.

use std::collections::HashMap;

use acme_core::{DrawCommand, ImageKey, Rgba8Image, Scene};
use acme_platform::PlatformKey;
use acme_style::prelude::Styled;
use acme_theme::Theme;
use acme_widgets::{WidgetNode, button, card, column, label, row, stack, text_input};

use super::edit::{Focus, char_cursor};
use super::geom::color;
use super::hit::Target;
use super::{FrameInput, Modifiers, UiEvent, UiRuntime};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Go,
    Other,
    Inner,
    Row,
    Text,
}

type Rt = UiRuntime<Msg>;

fn render_with(rt: &mut Rt, root: &WidgetNode<Msg>, images: &HashMap<String, Rgba8Image>) -> Scene {
    let theme = Theme::light();
    rt.render(
        root,
        FrameInput {
            logical_width: 800.0,
            logical_height: 600.0,
            scale_factor: 1.0,
            theme: &theme,
            images,
        },
    )
}

fn render(rt: &mut Rt, root: &WidgetNode<Msg>) -> Scene {
    render_with(rt, root, &HashMap::new())
}

fn rect_of(rt: &Rt, target: &Target) -> [f32; 4] {
    rt.frame
        .regions
        .iter()
        .find(|region| region.target == *target)
        .map(|region| region.rect)
        .unwrap_or_else(|| panic!("no region for {target:?}"))
}

fn has_region(rt: &Rt, key: &str) -> bool {
    rt.frame
        .regions
        .iter()
        .any(|region| region.target == Target::Button(key.to_owned()))
}

fn center(rect: [f32; 4]) -> (f32, f32) {
    (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0)
}

fn click_at(rt: &mut Rt, x: f32, y: f32) -> Vec<UiEvent<Msg>> {
    let _ = rt.pointer_button(true, x, y);
    rt.pointer_button(false, x, y)
}

fn click_center(rt: &mut Rt, target: &Target) -> Vec<UiEvent<Msg>> {
    let (x, y) = center(rect_of(rt, target));
    click_at(rt, x, y)
}

fn type_chars(rt: &mut Rt, text: &str) -> Vec<UiEvent<Msg>> {
    let mut events = Vec::new();
    for c in text.chars() {
        events.extend(rt.key(
            PlatformKey::Other,
            true,
            Modifiers::default(),
            Some(&c.to_string()),
        ));
    }
    events
}

fn text_changed(value: &str) -> UiEvent<Msg> {
    UiEvent::TextChanged {
        key: "name".to_owned(),
        value: value.to_owned(),
    }
}

fn input_view(value: &str) -> WidgetNode<Msg> {
    column::<Msg>()
        .child(text_input::<Msg>("name").value(value).on_change(Msg::Text))
        .build()
}

fn input_focus(rt: &mut Rt) {
    render(rt, &input_view(""));
    let _ = click_center(rt, &Target::Input("name".to_owned()));
    assert_eq!(rt.focused_input(), Some("name"));
}

fn quads(scene: &Scene) -> Vec<acme_core::QuadPrimitive> {
    scene
        .commands()
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Quad(quad) => Some(*quad),
            _ => None,
        })
        .collect()
}

#[test]
fn render_returns_valid_scene_with_background_clear() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(label::<Msg>("Hello"))
        .child(card::<Msg>().child(label::<Msg>("Card")).build())
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();

    let scene = render(&mut rt, &root);

    assert!(!scene.commands().is_empty());
    assert!(scene.validate().is_ok());
    assert_eq!(scene.clear_color(), color(Theme::light().colors.background));
}

#[test]
fn button_click_emits_its_message() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);

    let events = click_center(&mut rt, &Target::Button("go".to_owned()));

    assert_eq!(events, vec![UiEvent::Message(Msg::Go)]);
}

#[test]
fn press_on_a_and_release_on_b_emits_nothing() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("a", "A").on_click(Msg::Go))
        .child(button::<Msg>("b", "B").on_click(Msg::Other))
        .build();
    render(&mut rt, &root);
    let (ax, ay) = center(rect_of(&rt, &Target::Button("a".to_owned())));
    let (bx, by) = center(rect_of(&rt, &Target::Button("b".to_owned())));

    let _ = rt.pointer_button(true, ax, ay);
    let events = rt.pointer_button(false, bx, by);

    assert!(events.is_empty());
}

#[test]
fn disabled_button_emits_nothing() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").disabled(true).on_click(Msg::Go))
        .build();
    render(&mut rt, &root);

    // A disabled button records no region, so the click lands on empty space.
    assert!(
        !rt.frame
            .regions
            .iter()
            .any(|r| matches!(r.target, Target::Button(_)))
    );
    assert!(click_at(&mut rt, 10.0, 18.0).is_empty());
}

#[test]
fn container_on_click_fires_outside_its_children() {
    let mut rt = Rt::new();
    let root = row::<Msg>()
        .padding(20.0)
        .on_click(Msg::Row)
        .child(label::<Msg>("Tap"))
        .build();
    render(&mut rt, &root);
    let rect = rect_of(&rt, &Target::Container(0));

    let events = click_at(&mut rt, rect[0] + 5.0, rect[1] + 5.0);

    assert_eq!(events, vec![UiEvent::Message(Msg::Row)]);
}

#[test]
fn overlapping_stack_children_topmost_wins() {
    let mut rt = Rt::new();
    let root = stack::<Msg>()
        .child(button::<Msg>("a", "AAAA").on_click(Msg::Go))
        .child(button::<Msg>("b", "B").on_click(Msg::Other))
        .build();
    render(&mut rt, &root);
    let a = rect_of(&rt, &Target::Button("a".to_owned()));
    let b = rect_of(&rt, &Target::Button("b".to_owned()));
    assert_eq!(
        (a[0], a[1]),
        (0.0, 0.0),
        "stack children overlap at the origin"
    );
    assert_eq!(
        (b[0], b[1]),
        (0.0, 0.0),
        "stack children overlap at the origin"
    );

    let events = click_at(&mut rt, 5.0, 18.0);

    assert_eq!(events, vec![UiEvent::Message(Msg::Other)]);
}

#[test]
fn modal_absorbs_pointer_for_regions_painted_before_it() {
    let mut rt = Rt::new();
    let root = stack::<Msg>()
        .child(button::<Msg>("under", "Under").on_click(Msg::Go))
        .child(
            column::<Msg>()
                .key("modal:dlg")
                .size(800.0, 600.0)
                .padding(200.0)
                .child(button::<Msg>("inside", "In").on_click(Msg::Inner))
                .build(),
        )
        .build();
    render(&mut rt, &root);
    let under = center(rect_of(&rt, &Target::Button("under".to_owned())));
    let inside = center(rect_of(&rt, &Target::Button("inside".to_owned())));

    assert!(click_at(&mut rt, under.0, under.1).is_empty());
    assert_eq!(
        click_at(&mut rt, inside.0, inside.1),
        vec![UiEvent::Message(Msg::Inner)]
    );
}

#[test]
fn modal_restricts_tab_focus_to_its_children() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("outside", "Out").on_click(Msg::Go))
        .child(
            column::<Msg>()
                .key("modal:m")
                .child(button::<Msg>("in1", "In").on_click(Msg::Inner))
                .child(text_input::<Msg>("name").on_change(Msg::Text))
                .build(),
        )
        .build();
    render(&mut rt, &root);
    let tab = |rt: &mut Rt, shift: bool| {
        let _ = rt.key(
            PlatformKey::Tab,
            true,
            Modifiers {
                shift,
                ..Modifiers::default()
            },
            None,
        );
        rt.focus_target()
    };

    assert_eq!(
        rt.focus_target(),
        Some(Target::Button("in1".to_owned())),
        "the modal takes focus on its first frame"
    );
    assert_eq!(tab(&mut rt, false), Some(Target::Input("name".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Button("in1".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Input("name".to_owned())));
    assert_eq!(tab(&mut rt, true), Some(Target::Button("in1".to_owned())));
}

#[test]
fn container_without_message_is_pass_through() {
    let mut rt = Rt::new();
    let root = stack::<Msg>()
        .child(button::<Msg>("page", "Page").on_click(Msg::Go))
        .child(
            stack::<Msg>()
                .size(800.0, 600.0)
                .child(label::<Msg>("toast"))
                .build(),
        )
        .build();
    render(&mut rt, &root);
    let (x, y) = center(rect_of(&rt, &Target::Button("page".to_owned())));

    assert!(
        rt.frame
            .regions
            .iter()
            .all(|region| !matches!(region.target, Target::Container(_)))
    );
    assert!(rt.pointer_moved(x, y));
    assert_eq!(click_at(&mut rt, x, y), vec![UiEvent::Message(Msg::Go)]);
}

#[test]
fn clicking_focuses_input_and_emits_its_message() {
    let mut rt = Rt::new();
    render(&mut rt, &input_view(""));

    let events = click_center(&mut rt, &Target::Input("name".to_owned()));

    assert_eq!(rt.focused_input(), Some("name"));
    assert_eq!(events, vec![UiEvent::Message(Msg::Text)]);
}

#[test]
fn disabled_and_readonly_inputs_handle_focus_and_edits() {
    let mut rt = Rt::new();
    let disabled = column::<Msg>()
        .child(
            text_input::<Msg>("name")
                .disabled(true)
                .on_change(Msg::Text),
        )
        .build();
    render(&mut rt, &disabled);
    // Disabled input records no region; the click at its position focuses nothing.
    let _ = click_at(&mut rt, 100.0, 20.0);
    assert_eq!(rt.focused_input(), None);
    let readonly = column::<Msg>()
        .child(text_input::<Msg>("name").value("fixed").readonly(true))
        .build();
    render(&mut rt, &readonly);
    let _ = click_center(&mut rt, &Target::Input("name".to_owned()));
    assert_eq!(rt.focused_input(), Some("name"));

    assert!(type_chars(&mut rt, "x").is_empty());
}

#[test]
fn typing_builds_value_including_multibyte_text() {
    let mut rt = Rt::new();
    input_focus(&mut rt);

    assert_eq!(type_chars(&mut rt, "a"), vec![text_changed("a")]);
    render(&mut rt, &input_view("a"));
    assert_eq!(type_chars(&mut rt, "b"), vec![text_changed("ab")]);
    render(&mut rt, &input_view("ab"));
    assert_eq!(type_chars(&mut rt, "中"), vec![text_changed("ab中")]);
}

#[test]
fn backspace_removes_whole_chinese_character() {
    let mut rt = Rt::new();
    input_focus(&mut rt);
    let _ = type_chars(&mut rt, "ab中");
    render(&mut rt, &input_view("ab中"));

    let events = rt.key(PlatformKey::Backspace, true, Modifiers::default(), None);

    assert_eq!(events, vec![text_changed("ab")]);
}

#[test]
fn left_then_insert_places_text_in_the_middle() {
    let mut rt = Rt::new();
    input_focus(&mut rt);
    let _ = type_chars(&mut rt, "中a");
    render(&mut rt, &input_view("中a"));

    let _ = rt.key(PlatformKey::ArrowLeft, true, Modifiers::default(), None);
    let caret = match &rt.focus {
        Some(Focus::Input { state, .. }) => char_cursor(state),
        _ => usize::MAX,
    };
    assert_eq!(caret, 1, "caret should sit between the two characters");
    assert_eq!(type_chars(&mut rt, "b"), vec![text_changed("中ba")]);
}

#[test]
fn enter_submits_and_escape_blurs_input_before_cancelling() {
    let mut rt = Rt::new();
    input_focus(&mut rt);

    let submit = rt.key(PlatformKey::Enter, true, Modifiers::default(), None);
    let blur = rt.key(PlatformKey::Escape, true, Modifiers::default(), None);

    assert_eq!(
        submit,
        vec![UiEvent::Submit {
            key: "name".to_owned()
        }]
    );
    assert!(blur.is_empty(), "escape in a focused input only blurs");
    assert_eq!(rt.focused_input(), None);
    assert_eq!(
        rt.key(PlatformKey::Escape, true, Modifiers::default(), None),
        vec![UiEvent::Cancel],
        "escape with nothing focused cancels"
    );
}

#[test]
fn escape_with_focused_button_cancels() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);
    let _ = click_center(&mut rt, &Target::Button("go".to_owned()));

    assert_eq!(
        rt.key(PlatformKey::Escape, true, Modifiers::default(), None),
        vec![UiEvent::Cancel]
    );
    assert_eq!(rt.focus_target(), None);
}

#[test]
fn ime_commit_and_paste_flatten_line_breaks_and_drop_controls() {
    let mut rt = Rt::new();
    input_focus(&mut rt);

    assert_eq!(
        rt.ime_commit("你\r\n好\t\u{7}!"),
        vec![text_changed("你 好 !")]
    );

    let mut state = acme_textinput::TextInputState::new();
    super::edit::paste_into(&mut state, "a\r\nb\tc\u{1b}\rd\ne");
    assert_eq!(state.text, "a b c d e");
}

#[test]
fn modal_appearing_takes_focus_and_background_buttons_stay_inert() {
    let mut rt = Rt::new();
    let background = column::<Msg>()
        .child(button::<Msg>("bg", "Bg").on_click(Msg::Go))
        .build();
    render(&mut rt, &background);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    let with_modal = column::<Msg>()
        .child(button::<Msg>("bg", "Bg").on_click(Msg::Go))
        .child(
            column::<Msg>()
                .key("modal:dlg")
                .child(button::<Msg>("ok", "OK").on_click(Msg::Inner))
                .build(),
        )
        .build();
    render(&mut rt, &with_modal);

    assert_eq!(
        rt.key(PlatformKey::Enter, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Inner)],
        "focus moved into the modal; the background button must not fire"
    );
}

#[test]
fn modal_blocks_typing_into_background_input() {
    let mut rt = Rt::new();
    let background = column::<Msg>()
        .child(text_input::<Msg>("bgname").on_change(Msg::Text))
        .build();
    render(&mut rt, &background);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    let with_modal = column::<Msg>()
        .child(text_input::<Msg>("bgname").on_change(Msg::Text))
        .child(
            column::<Msg>()
                .key("modal:dlg")
                .child(button::<Msg>("ok", "OK").on_click(Msg::Inner))
                .build(),
        )
        .build();
    render(&mut rt, &with_modal);

    assert!(type_chars(&mut rt, "x").is_empty());
}

fn two_axis_list() -> WidgetNode<Msg> {
    let mut list = column::<Msg>()
        .key("list")
        .size(300.0, 100.0)
        .overflow_scroll();
    list = list.child(
        row::<Msg>()
            .size(600.0, 30.0)
            .child(button::<Msg>("wide", "Wide").on_click(Msg::Go))
            .build(),
    );
    for index in 0..8 {
        list = list.child(
            button::<Msg>(format!("t{index}"), format!("Tall {index}")).on_click(Msg::Other),
        );
    }
    list.build()
}

#[test]
fn scroll_region_scrolls_both_axes_by_wheel_axis() {
    let mut rt = Rt::new();
    let root = two_axis_list();
    render(&mut rt, &root);
    let (x, y) = center(rt.frame.scroll[0].viewport);
    let wide0 = rect_of(&rt, &Target::Button("wide".to_owned()))[0];
    let first0 = rect_of(&rt, &Target::Button("t0".to_owned()))[1];

    assert!(
        rt.scroll(-30.0, 0.0, false, x, y),
        "horizontal wheel scrolls x"
    );
    render(&mut rt, &root);
    assert_eq!(
        rect_of(&rt, &Target::Button("wide".to_owned()))[0],
        wide0 - 30.0
    );
    assert_eq!(rect_of(&rt, &Target::Button("t0".to_owned()))[1], first0);

    assert!(
        rt.scroll(0.0, -20.0, false, x, y),
        "vertical wheel scrolls y"
    );
    render(&mut rt, &root);
    assert_eq!(
        rect_of(&rt, &Target::Button("t0".to_owned()))[1],
        first0 - 20.0
    );
    assert_eq!(
        rect_of(&rt, &Target::Button("wide".to_owned()))[0],
        wide0 - 30.0
    );
}

#[test]
fn vertical_wheel_pans_a_region_that_only_overflows_horizontally() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .key("strip")
        .size(300.0, 100.0)
        .overflow_scroll()
        .child(
            row::<Msg>()
                .size(600.0, 30.0)
                .child(button::<Msg>("sw", "Wide").on_click(Msg::Go))
                .build(),
        )
        .build();
    render(&mut rt, &root);
    let (x, y) = center(rt.frame.scroll[0].viewport);
    let sw0 = rect_of(&rt, &Target::Button("sw".to_owned()))[0];

    assert!(rt.scroll(0.0, -20.0, false, x, y));
    render(&mut rt, &root);

    assert_eq!(
        rect_of(&rt, &Target::Button("sw".to_owned()))[0],
        sw0 - 20.0
    );
}

#[test]
fn keyed_clickable_container_takes_keyboard_focus_and_activates() {
    let mut rt = Rt::new();
    let ring = color(Theme::light().colors.ring);
    let has_ring = |scene: &Scene| {
        quads(scene)
            .iter()
            .any(|q| q.border_width == 2.0 && q.border_color == ring)
    };
    let root = column::<Msg>()
        .child(
            row::<Msg>()
                .key("mail:1")
                .padding(10.0)
                .on_click(Msg::Row)
                .child(label::<Msg>("Mail"))
                .build(),
        )
        .build();
    render(&mut rt, &root);

    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    assert!(
        has_ring(&render(&mut rt, &root)),
        "keyboard focus draws the ring"
    );
    assert_eq!(
        rt.key(PlatformKey::Enter, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Row)]
    );
    assert_eq!(
        rt.key(PlatformKey::Space, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Row)]
    );

    let rect = rt
        .frame
        .regions
        .iter()
        .find(|region| region.message.is_some())
        .map(|region| region.rect)
        .unwrap_or_else(|| panic!("container region missing"));
    let _ = click_at(&mut rt, rect[0] + 2.0, rect[1] + 2.0);
    assert!(
        !has_ring(&render(&mut rt, &root)),
        "pointer focus draws no ring"
    );
    assert_eq!(
        rt.key(PlatformKey::Enter, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Row)],
        "a click keeps the container focused"
    );
}

#[test]
fn off_screen_text_is_not_shaped() {
    let mut rt = Rt::new();
    render(&mut rt, &inbox_list(200));

    assert!(rt.text.shape_calls > 0, "visible rows are shaped");
    assert!(
        rt.text.shape_calls < 150,
        "600 labels, only the visible rows should shape; got {}",
        rt.text.shape_calls
    );
}

#[test]
fn identical_second_frame_reuses_shaped_text() {
    let mut rt = Rt::new();
    let root = inbox_list(20);
    render(&mut rt, &root);
    let first = rt.text.shape_calls;
    assert!(first > 0);

    render(&mut rt, &root);

    assert_eq!(
        rt.text.shape_calls, first,
        "no new shaping for an identical frame"
    );
}

#[test]
fn ime_commit_inserts_text_and_clears_preedit() {
    let mut rt = Rt::new();
    input_focus(&mut rt);

    assert!(rt.ime_preedit("ni"));
    assert!(!rt.ime_preedit("ni"));
    assert_eq!(rt.ime_commit("你"), vec![text_changed("你")]);
    assert!(rt.ime_commit("").is_empty());
    render(&mut rt, &input_view("你"));
    assert!(
        rt.ime_cursor_area().is_some(),
        "caret rect is recorded by render"
    );
}

#[test]
fn app_side_value_change_is_adopted_on_next_render() {
    let mut rt = Rt::new();
    input_focus(&mut rt);
    assert_eq!(type_chars(&mut rt, "x"), vec![text_changed("x")]);

    render(&mut rt, &input_view("server"));
    // The caret keeps its byte offset (after "x"'s position), clamped to the new text.
    assert_eq!(type_chars(&mut rt, "y"), vec![text_changed("syerver")]);
}

fn scroll_list() -> WidgetNode<Msg> {
    let mut list = column::<Msg>()
        .key("list")
        .size(300.0, 100.0)
        .overflow_scroll();
    for index in 0..10 {
        let msg = if index == 0 { Msg::Go } else { Msg::Other };
        list =
            list.child(button::<Msg>(format!("b{index}"), format!("Item {index}")).on_click(msg));
    }
    list.build()
}

#[test]
fn scroll_region_scrolls_clamps_and_persists() {
    let mut rt = Rt::new();
    let root = scroll_list();
    render(&mut rt, &root);
    let viewport = rt.frame.scroll[0].viewport;
    let (x, y) = center(viewport);

    assert!(rt.scroll(0.0, -100.0, false, x, y));
    assert_eq!(rt.scroll_offsets.get("list").map(|o| o.y), Some(100.0));
    assert!(rt.scroll(0.0, -1000.0, false, x, y));
    assert_eq!(
        rt.scroll_offsets.get("list").map(|o| o.y),
        Some(260.0),
        "clamped to content - viewport"
    );
    render(&mut rt, &root);
    assert_eq!(
        rt.scroll_offsets.get("list").map(|o| o.y),
        Some(260.0),
        "offset persists across frames"
    );
    assert!(rt.scroll(0.0, 1000.0, false, x, y));
    assert_eq!(rt.scroll_offsets.get("list").map(|o| o.y), Some(0.0));
}

#[test]
fn button_scrolled_out_of_view_is_not_clickable() {
    let mut rt = Rt::new();
    let root = scroll_list();
    render(&mut rt, &root);
    assert!(
        !has_region(&rt, "b9"),
        "b9 is below the viewport and has no hit region"
    );

    let viewport = rt.frame.scroll[0].viewport;
    let (vx, vy) = center(viewport);
    assert!(rt.scroll(0.0, -1000.0, false, vx, vy));
    render(&mut rt, &root);
    let (x, y) = center(rect_of(&rt, &Target::Button("b9".to_owned())));
    assert_eq!(click_at(&mut rt, x, y), vec![UiEvent::Message(Msg::Other)]);
    assert!(!has_region(&rt, "b0"), "b0 is above the viewport");
}

#[test]
fn tab_cycles_focus_in_paint_order() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("a", "A").on_click(Msg::Go))
        .child(text_input::<Msg>("name").on_change(Msg::Text))
        .child(button::<Msg>("c", "C").on_click(Msg::Other))
        .build();
    render(&mut rt, &root);
    let tab = |rt: &mut Rt, shift: bool| {
        let _ = rt.key(
            PlatformKey::Tab,
            true,
            Modifiers {
                shift,
                ..Modifiers::default()
            },
            None,
        );
        rt.focus_target()
    };

    assert_eq!(tab(&mut rt, false), Some(Target::Button("a".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Input("name".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Button("c".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Button("a".to_owned())));
    assert_eq!(tab(&mut rt, true), Some(Target::Button("c".to_owned())));
}

#[test]
fn enter_and_space_activate_focused_button() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);

    assert_eq!(
        rt.key(PlatformKey::Enter, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Go)]
    );
    assert_eq!(
        rt.key(PlatformKey::Space, true, Modifiers::default(), None),
        vec![UiEvent::Message(Msg::Go)]
    );
}

#[test]
fn image_slot_draws_image_when_provided() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(
            column::<Msg>()
                .key("img:preview")
                .size(200.0, 100.0)
                .build(),
        )
        .build();
    let image = Rgba8Image::new(ImageKey::new(1), 0, 2, 2, vec![255u8; 16]).expect("valid image");
    let mut images = HashMap::new();
    images.insert("img:preview".to_owned(), image);

    let with_image = render_with(&mut rt, &root, &images);
    let without_image = render(&mut rt, &root);

    assert!(
        with_image
            .commands()
            .iter()
            .any(|command| matches!(command, DrawCommand::Image(_)))
    );
    assert!(
        !without_image
            .commands()
            .iter()
            .any(|command| matches!(command, DrawCommand::Image(_)))
    );
}

#[test]
fn clickable_container_shows_hover_and_pressed_overlay() {
    let mut rt = Rt::new();
    let root = row::<Msg>()
        .padding(20.0)
        .on_click(Msg::Row)
        .child(label::<Msg>("Tap"))
        .build();
    render(&mut rt, &root);
    let rect = rect_of(&rt, &Target::Container(0));
    let theme = Theme::light();
    let hover_color = color(theme.colors.ghost_hover);
    let pressed_color = color(theme.colors.ghost_pressed);
    let fills = |scene: &Scene| quads(scene).iter().map(|q| q.color).collect::<Vec<_>>();

    assert!(!fills(&render(&mut rt, &root)).contains(&hover_color));
    assert!(rt.pointer_moved(rect[0] + 5.0, rect[1] + 5.0));
    let hovered = render(&mut rt, &root);
    assert!(fills(&hovered).contains(&hover_color));
    let _ = rt.pointer_button(true, rect[0] + 5.0, rect[1] + 5.0);
    let pressed = render(&mut rt, &root);
    assert!(fills(&pressed).contains(&pressed_color));
}

#[test]
fn focus_ring_follows_keyboard_focus_not_pointer_focus() {
    let mut rt = Rt::new();
    let ring = color(Theme::light().colors.ring);
    let has_ring = |scene: &Scene| {
        quads(scene)
            .iter()
            .any(|q| q.border_width == 2.0 && q.border_color == ring)
    };
    let idle = render(&mut rt, &input_view(""));
    assert!(!has_ring(&idle));

    input_focus(&mut rt);
    assert!(
        !has_ring(&render(&mut rt, &input_view(""))),
        "a pointer click focuses without the ring"
    );
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    assert!(has_ring(&render(&mut rt, &input_view(""))));

    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    assert!(has_ring(&render(&mut rt, &root)));
    let _ = click_center(&mut rt, &Target::Button("go".to_owned()));
    assert!(!has_ring(&render(&mut rt, &root)), "a click hides the ring");
}

#[test]
fn button_width_comes_from_its_label() {
    let mut rt = Rt::new();
    // Buttons stretch across a Column (cross-axis stretch); intrinsic width applies in a Row.
    let root = row::<Msg>()
        .child(button::<Msg>("short", "Go").on_click(Msg::Go))
        .child(button::<Msg>("long", "A considerably longer label").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);
    let short = rect_of(&rt, &Target::Button("short".to_owned()))[2];
    let long = rect_of(&rt, &Target::Button("long".to_owned()))[2];

    assert!(short > 20.0, "padding alone is 20px; got {short}");
    assert!(long > short + 40.0, "long={long} short={short}");
}

#[test]
fn non_finite_inputs_never_panic() {
    let mut rt = Rt::new();
    let theme = Theme::light();
    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    let scene = rt.render(
        &root,
        FrameInput {
            logical_width: f32::NAN,
            logical_height: f32::INFINITY,
            scale_factor: f32::NAN,
            theme: &theme,
            images: &HashMap::new(),
        },
    );
    assert!(scene.validate().is_ok());

    assert!(!rt.scroll(f32::NAN, f32::NAN, true, f32::NAN, 0.0));
    let _ = rt.pointer_moved(f32::NAN, f32::NAN);
    assert!(rt.pointer_button(true, f32::NAN, f32::NAN).is_empty());
    rt.clear_gpu_caches();
}

#[test]
fn ctrl_a_selects_all_so_typing_replaces_the_value() {
    let mut rt = Rt::new();
    input_focus(&mut rt);
    let _ = type_chars(&mut rt, "abc");
    render(&mut rt, &input_view("abc"));
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };

    assert!(
        rt.key(PlatformKey::Other, true, ctrl, Some("\u{1}"))
            .is_empty()
    );
    assert_eq!(type_chars(&mut rt, "z"), vec![text_changed("z")]);
}

#[test]
fn reset_scroll_returns_region_to_top() {
    let mut rt = Rt::new();
    render(&mut rt, &scroll_list());
    let (x, y) = center(rt.frame.scroll[0].viewport);
    assert!(rt.scroll(0.0, -1000.0, false, x, y));

    rt.reset_scroll("list");
    render(&mut rt, &scroll_list());

    assert_eq!(rt.scroll_offsets.get("list").map(|o| o.y), Some(0.0));
}

/// Keyed, scrollable inbox-like list: one clickable row per mail with three labels.
fn inbox_list(rows: usize) -> WidgetNode<Msg> {
    let mut list = column::<Msg>()
        .key("inbox")
        .size(640.0, 480.0)
        .overflow_scroll();
    for index in 0..rows {
        list = list.child(
            row::<Msg>()
                .key(format!("mail:{index}"))
                .padding(6.0)
                .on_click(Msg::Row)
                .child(label::<Msg>(format!("Sender {index}")))
                .child(label::<Msg>(format!("Subject line number {index}")))
                .child(label::<Msg>(format!("2026-11-{:02}", index % 28 + 1)))
                .build(),
        );
    }
    list.build()
}

/// Micro-benchmark: `cargo test --release --lib ui::runtime -- --ignored --nocapture bench_render`.
#[test]
#[ignore]
fn bench_render_200_row_column() {
    for rows in [50usize, 200] {
        let root = inbox_list(rows);
        let mut rt = Rt::new();
        let start = std::time::Instant::now();
        render(&mut rt, &root);
        let cold = start.elapsed();
        let mut warm = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            let scene = render(&mut rt, &root);
            warm.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(scene.validate().is_ok());
        }
        warm.sort_by(f64::total_cmp);
        let avg = warm.iter().sum::<f64>() / warm.len() as f64;
        println!(
            "BENCH rows={rows} cold_ms={:.2} warm_avg_ms={avg:.2} warm_min_ms={:.2} warm_max_ms={:.2} commands={}",
            cold.as_secs_f64() * 1000.0,
            warm[0],
            warm[warm.len() - 1],
            render(&mut rt, &root).commands().len(),
        );
    }
}

#[test]
fn focused_button_scrolled_out_of_view_keeps_focus() {
    let mut rt = Rt::new();
    let root = scroll_list();
    render(&mut rt, &root);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    assert_eq!(rt.focus_target(), Some(Target::Button("b0".to_owned())));
    let (x, y) = center(rt.frame.scroll[0].viewport);
    assert!(rt.scroll(0.0, -1000.0, false, x, y));

    render(&mut rt, &root);

    assert!(!has_region(&rt, "b0"), "b0 is above the viewport");
    assert_eq!(
        rt.focus_target(),
        Some(Target::Button("b0".to_owned())),
        "focus survives culling"
    );
}

#[test]
fn nested_scroll_offset_survives_being_scrolled_out_of_view() {
    let mut rt = Rt::new();
    let mut inner = column::<Msg>()
        .key("inner")
        .size(300.0, 100.0)
        .overflow_scroll();
    for index in 0..5 {
        inner = inner
            .child(button::<Msg>(format!("i{index}"), format!("Inner {index}")).on_click(Msg::Go));
    }
    let root = column::<Msg>()
        .key("outer")
        .size(400.0, 200.0)
        .overflow_scroll()
        .child(inner.build())
        .child(column::<Msg>().size(400.0, 400.0).build())
        .build();
    render(&mut rt, &root);
    let inner_view = rt
        .frame
        .scroll
        .iter()
        .find(|region| region.key == "inner")
        .map(|region| region.viewport)
        .unwrap_or_else(|| panic!("inner scroll region missing"));
    let (ix, iy) = center(inner_view);
    assert!(rt.scroll(0.0, -40.0, false, ix, iy));
    let outer_view = rt
        .frame
        .scroll
        .iter()
        .find(|region| region.key == "outer")
        .map(|region| region.viewport)
        .unwrap_or_else(|| panic!("outer scroll region missing"));
    assert!(rt.scroll(
        0.0,
        -300.0,
        false,
        outer_view[0] + 5.0,
        outer_view[1] + 180.0
    ));

    render(&mut rt, &root);

    assert!(
        !rt.frame.scroll.iter().any(|region| region.key == "inner"),
        "inner region is scrolled out of the outer viewport"
    );
    assert_eq!(
        rt.scroll_offsets.get("inner").map(|o| o.y),
        Some(40.0),
        "hidden nested offsets are kept"
    );
}

#[test]
fn keyed_container_hover_overlay_still_works() {
    let mut rt = Rt::new();
    let root = column::<Msg>()
        .child(
            row::<Msg>()
                .key("mail:1")
                .padding(20.0)
                .on_click(Msg::Row)
                .child(label::<Msg>("Tap"))
                .build(),
        )
        .build();
    render(&mut rt, &root);
    let rect = rt
        .frame
        .regions
        .iter()
        .find(|region| region.message.is_some())
        .map(|region| region.rect)
        .unwrap_or_else(|| panic!("container region missing"));
    let hover_color = color(Theme::light().colors.ghost_hover);
    let fills = |scene: &Scene| quads(scene).iter().map(|q| q.color).collect::<Vec<_>>();

    assert!(rt.pointer_moved(rect[0] + 5.0, rect[1] + 5.0));

    assert!(fills(&render(&mut rt, &root)).contains(&hover_color));
}

/// Floor measurement: the framework layout pass alone (not paint).
#[test]
#[ignore]
fn bench_layout_floor_200_rows() {
    use acme_core::NodeId;
    use acme_layout::WidgetLayoutContext;
    let theme = Theme::light();
    let context = WidgetLayoutContext {
        body_font_size: theme.typography.body,
        body_line_height: theme.typography.body * theme.typography.line_height,
        label_font_size: theme.typography.label,
        control_height: theme.control_height(3),
        scale_factor: 1.0,
    };
    for rows in [50usize, 200] {
        let root = inbox_list(rows);
        let mut rt = Rt::new();
        let tree = root.to_layout_with_context(NodeId::new(1), &context);
        let mut times = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            let snapshot = rt
                .layout
                .compute_with_text(&tree, (800.0, 600.0), &mut rt.fonts, 1.0);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(snapshot.is_ok());
        }
        times.sort_by(f64::total_cmp);
        println!(
            "LAYOUT_FLOOR rows={rows} warm_min_ms={:.2} warm_median_ms={:.2}",
            times[0],
            times[times.len() / 2]
        );
    }
}
