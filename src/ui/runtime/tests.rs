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

    assert_eq!(tab(&mut rt, false), Some(Target::Button("in1".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Input("name".to_owned())));
    assert_eq!(tab(&mut rt, false), Some(Target::Button("in1".to_owned())));
    assert_eq!(tab(&mut rt, true), Some(Target::Input("name".to_owned())));
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
fn enter_submits_and_escape_cancels_and_blurs() {
    let mut rt = Rt::new();
    input_focus(&mut rt);

    let submit = rt.key(PlatformKey::Enter, true, Modifiers::default(), None);
    let cancel = rt.key(PlatformKey::Escape, true, Modifiers::default(), None);

    assert_eq!(
        submit,
        vec![UiEvent::Submit {
            key: "name".to_owned()
        }]
    );
    assert_eq!(cancel, vec![UiEvent::Cancel]);
    assert_eq!(rt.focused_input(), None);
    assert_eq!(
        rt.key(PlatformKey::Escape, true, Modifiers::default(), None),
        vec![UiEvent::Cancel],
        "escape with nothing focused still cancels"
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
    assert_eq!(rt.scroll_offsets.get("list"), Some(&100.0));
    assert!(rt.scroll(0.0, -1000.0, false, x, y));
    assert_eq!(
        rt.scroll_offsets.get("list"),
        Some(&260.0),
        "clamped to content - viewport"
    );
    render(&mut rt, &root);
    assert_eq!(
        rt.scroll_offsets.get("list"),
        Some(&260.0),
        "offset persists across frames"
    );
    assert!(rt.scroll(0.0, 1000.0, false, x, y));
    assert_eq!(rt.scroll_offsets.get("list"), Some(&0.0));
}

#[test]
fn button_scrolled_out_of_view_is_not_clickable() {
    let mut rt = Rt::new();
    let root = scroll_list();
    render(&mut rt, &root);
    let (x, y) = center(rect_of(&rt, &Target::Button("b9".to_owned())));
    assert!(
        click_at(&mut rt, x, y).is_empty(),
        "b9 is below the viewport"
    );

    let viewport = rt.frame.scroll[0].viewport;
    let (vx, vy) = center(viewport);
    assert!(rt.scroll(0.0, -1000.0, false, vx, vy));
    render(&mut rt, &root);
    let (x, y) = center(rect_of(&rt, &Target::Button("b9".to_owned())));
    assert_eq!(click_at(&mut rt, x, y), vec![UiEvent::Message(Msg::Other)]);
    let (x, y) = center(rect_of(&rt, &Target::Button("b0".to_owned())));
    assert!(
        click_at(&mut rt, x, y).is_empty(),
        "b0 is above the viewport"
    );
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
fn focused_input_and_button_draw_two_pixel_ring() {
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
    let focused = render(&mut rt, &input_view(""));
    assert!(has_ring(&focused));

    let root = column::<Msg>()
        .child(button::<Msg>("go", "Go").on_click(Msg::Go))
        .build();
    render(&mut rt, &root);
    let _ = rt.key(PlatformKey::Tab, true, Modifiers::default(), None);
    assert!(has_ring(&render(&mut rt, &root)));
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

    assert_eq!(rt.scroll_offsets.get("list"), Some(&0.0));
}
