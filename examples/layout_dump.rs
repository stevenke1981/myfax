//! Developer tool: prints the layout rectangles of `views::root` for a sample state.
//! Usage: `cargo run --example layout_dump -- <depth> [name-filter]`.
use acme_core::NodeId;
use acme_layout::{LayoutEngine, LayoutNode, LayoutSnapshot, WidgetLayoutContext};
use acme_text::FontSystem;
use acme_widgets::WidgetNode;
use myfax::app::{AppState, Msg};
use myfax::config::Settings;

fn walk(w: &WidgetNode<Msg>, l: &LayoutNode, s: &LayoutSnapshot, depth: usize, max: usize) {
    if depth > max {
        return;
    }
    let r = s.get(l.id).copied();
    let name = match w {
        WidgetNode::Label(x) => format!("Label({:?})", x.text.chars().take(20).collect::<String>()),
        WidgetNode::Button(b) => format!("Button({:?})", b.label),
        WidgetNode::TextInput(t) => format!("Input({:?})", t.key),
        WidgetNode::Row(c) => format!("Row{:?}", c.key),
        WidgetNode::Column(c) => format!("Col{:?}", c.key),
        WidgetNode::Stack(c) => format!("Stack{:?}", c.key),
        WidgetNode::Card(_) => "Card".into(),
        other => format!("{:?}", std::mem::discriminant(other)),
    };
    if let Some(r) = r {
        println!(
            "{}{} x={:.0} y={:.0} w={:.0} h={:.0}",
            "  ".repeat(depth),
            name,
            r.x,
            r.y,
            r.width,
            r.height
        );
    }
    for (cw, cl) in w.children().iter().zip(&l.children) {
        walk(cw, cl, s, depth + 1, max);
    }
}

fn main() {
    let max: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(6);
    let mut state = AppState::new(Settings::default());
    let rec = myfax::model::FaxRecord {
        id: 1,
        received_at: chrono::Local::now(),
        sender_number: Some("0912345678".into()),
        sender_name: None,
        pages: 1,
        file_path: "C:/x/a.pdf".into(),
        file_size: 6000,
        sha256: "a".into(),
        original_name: "a.pdf".into(),
        source: myfax::model::FaxSource::Manual,
        is_read: true,
        is_starred: false,
        tags: vec![],
        note: String::new(),
        deleted_at: None,
        imported_at: chrono::Local::now(),
    };
    state.results.items = vec![rec.clone()];
    state.results.total = 1;
    state.selected = Some(rec);
    let theme = acme_theme::Theme::light();
    let view = myfax::ui::views::root(&state);
    let ctx = WidgetLayoutContext {
        body_font_size: theme.typography.body,
        body_line_height: theme.typography.body * theme.typography.line_height,
        label_font_size: theme.typography.label,
        control_height: 36.0,
        scale_factor: 1.0,
    };
    let root = view.to_layout_with_context(NodeId::new(1), &ctx);
    let mut fonts = FontSystem::new();
    let snap = LayoutEngine::new()
        .compute_with_text(&root, (1280.0, 800.0), &mut fonts, 1.0)
        .unwrap();
    walk(&view, &root, &snap, 0, max);
}
