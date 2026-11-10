use super::*;
use crate::config::Settings;
use acme_widgets::{Card, ScrollView};
use std::collections::HashSet;

#[derive(Default)]
struct Walk {
    keys: Vec<String>,
    msgs: Vec<Msg>,
    buttons: Vec<(String, ButtonVariant)>,
}

fn walk(node: &WidgetNode<Msg>, out: &mut Walk) {
    if let Some(key) = node.key() {
        out.keys.push(key.as_str().to_string());
    }
    match node {
        WidgetNode::Row(c) | WidgetNode::Column(c) | WidgetNode::Stack(c) => {
            if let Some(msg) = &c.message {
                out.msgs.push(msg.clone());
            }
            c.children.iter().for_each(|child| walk(child, out));
        }
        WidgetNode::Card(Card { children, .. }) => {
            children.iter().for_each(|child| walk(child, out));
        }
        WidgetNode::ScrollView(ScrollView { children, .. }) => {
            children.iter().for_each(|child| walk(child, out));
        }
        WidgetNode::Button(b) => {
            out.buttons.push((b.key.as_str().to_string(), b.variant));
            if let Some(msg) = b.activate() {
                out.msgs.push(msg.clone());
            }
        }
        _ => {}
    }
}

fn collect(state: &AppState) -> Walk {
    let tree = view(state);
    let mut out = Walk::default();
    walk(&tree, &mut out);
    out
}

fn day(year: i32, month: u32, date: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, date).unwrap()
}

fn bucket(date: NaiveDate, count: u64) -> DateBucket {
    DateBucket { date, count }
}

fn today() -> NaiveDate {
    day(2026, 11, 10)
}

fn heights(per_day: &[DateBucket], days: u32) -> Vec<f32> {
    bar_heights(per_day, days, today(), 100.0)
        .into_iter()
        .map(|(_, h)| h)
        .collect()
}

#[test]
fn bar_heights_zero_fill_missing_days() {
    let per_day = [bucket(day(2026, 11, 7), 4)];
    let bars = bar_heights(&per_day, 7, today(), 100.0);
    assert_eq!(bars.len(), 7);
    assert_eq!(bars.first().map(|(d, _)| *d), Some(day(2026, 11, 4)));
    assert_eq!(bars.last().map(|(d, _)| *d), Some(day(2026, 11, 10)));
    assert_eq!(
        heights(&per_day, 7),
        vec![0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn bar_heights_scale_to_the_largest_day() {
    let per_day = [bucket(day(2026, 11, 9), 2), bucket(day(2026, 11, 10), 4)];
    assert_eq!(
        heights(&per_day, 7),
        vec![0.0, 0.0, 0.0, 0.0, 0.0, 50.0, 100.0]
    );
}

#[test]
fn tiny_counts_keep_a_visible_bar() {
    let per_day = [bucket(day(2026, 11, 9), 1), bucket(day(2026, 11, 10), 1000)];
    assert_eq!(heights(&per_day, 7)[5], MIN_BAR_H);
}

#[test]
fn days_outside_the_range_are_ignored() {
    let per_day = [bucket(day(2026, 10, 1), 9)];
    assert!(heights(&per_day, 7).iter().all(|h| *h == 0.0));
}

#[test]
fn duplicate_dates_are_summed() {
    let per_day = [bucket(day(2026, 11, 10), 2), bucket(day(2026, 11, 10), 3)];
    let counts = bucket_counts(&per_day, 7, today());
    assert_eq!(counts.last().map(|(_, c)| *c), Some(5));
}

#[test]
fn year_range_aggregates_into_twelve_months() {
    let per_day = [
        bucket(day(2026, 11, 2), 3),
        bucket(day(2026, 10, 15), 5),
        bucket(day(2026, 1, 5), 2),
        bucket(day(2025, 10, 20), 7),
    ];
    let bars = bar_heights(&per_day, 365, today(), 100.0);
    assert_eq!(bars.len(), 12);
    assert_eq!(bars.first().map(|(d, _)| *d), Some(day(2025, 12, 1)));
    assert_eq!(bars.last().map(|(d, _)| *d), Some(day(2026, 11, 1)));
    let counts: Vec<u64> = bucket_counts(&per_day, 365, today())
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    assert_eq!(counts, vec![0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 5, 3]);
    let pixels: Vec<f32> = bars.into_iter().map(|(_, h)| h).collect();
    assert_eq!(pixels[1], 40.0);
    assert_eq!(pixels[10], 100.0);
    assert_eq!(pixels[11], 60.0);
}

#[test]
fn label_policy_thins_thirty_day_axis() {
    assert!(show_label(0, 30, 30));
    assert!(!show_label(3, 30, 30));
    assert!(show_label(5, 30, 30));
    assert!(show_label(29, 30, 30));
    assert!(show_label(2, 7, 7));
    assert!(show_label(4, 12, 365));
}

#[test]
fn bucket_labels_match_range() {
    assert_eq!(bucket_label(day(2026, 11, 5), 7), "11-05");
    assert_eq!(bucket_label(day(2026, 11, 5), 30), "05");
    assert_eq!(bucket_label(day(2026, 11, 5), 365), "11");
}

#[test]
fn bar_geometry_matches_spec() {
    assert_eq!(bar_geometry(7), (40.0, 12.0));
    assert_eq!(bar_geometry(30), (16.0, 6.0));
    assert_eq!(bar_geometry(365), (36.0, 12.0));
}

#[test]
fn scale_heights_keeps_zeros() {
    assert_eq!(scale_heights(&[0, 0], 100.0), vec![0.0, 0.0]);
    assert!(scale_heights(&[], 100.0).is_empty());
}

#[test]
fn averages_use_range_length_and_guard_zero() {
    assert_eq!(avg_per_day_text(70, 7), "10.0");
    assert_eq!(avg_per_day_text(10, 30), "0.3");
    assert_eq!(avg_per_day_text(0, 365), "0.0");
    assert_eq!(avg_pages_text(0, 0), "—");
    assert_eq!(avg_pages_text(7, 2), "3.5");
}

#[test]
fn busiest_day_prefers_earliest_on_tie_and_ignores_zero() {
    let per_day = [
        bucket(day(2026, 11, 1), 0),
        bucket(day(2026, 11, 3), 4),
        bucket(day(2026, 11, 8), 4),
        bucket(day(2026, 11, 9), 1),
    ];
    assert_eq!(busiest_day(&per_day), Some(bucket(day(2026, 11, 3), 4)));
    assert_eq!(busiest_day(&[bucket(day(2026, 11, 1), 0)]), None);
}

#[test]
fn sender_bar_width_scales_and_clamps() {
    assert_eq!(sender_bar_width(10, 10), SENDER_BAR_MAX_W);
    assert_eq!(sender_bar_width(1, 100), SENDER_BAR_MIN_W);
    assert_eq!(sender_bar_width(0, 0), SENDER_BAR_MIN_W);
    assert_eq!(sender_bar_width(5, 10), 80.0);
}

#[test]
fn range_buttons_emit_set_stats_range_and_highlight_active() {
    let mut state = AppState::new(Settings::default());
    state.stats_range = StatsRange::Last30Days;
    let out = collect(&state);
    for range in [
        StatsRange::Last7Days,
        StatsRange::Last30Days,
        StatsRange::Last365Days,
    ] {
        assert!(out.msgs.contains(&Msg::SetStatsRange(range)));
    }
    let variant = |key: &str| out.buttons.iter().find(|(k, _)| k == key).map(|(_, v)| *v);
    assert_eq!(variant("btn.range.30d"), Some(ButtonVariant::Primary));
    assert_eq!(variant("btn.range.7d"), Some(ButtonVariant::Secondary));
    assert_eq!(variant("btn.range.365d"), Some(ButtonVariant::Secondary));
}

#[test]
fn top_senders_render_ranked_rows_and_empty_state() {
    let mut state = AppState::new(Settings::default());
    state.stats.top_senders = vec![("ACME".into(), 3), ("Bob".into(), 1)];
    let out = collect(&state);
    assert!(out.keys.contains(&"top.0".to_string()));
    assert!(out.keys.contains(&"top.1".to_string()));

    let empty = collect(&AppState::new(Settings::default()));
    assert!(!empty.keys.iter().any(|k| k.starts_with("top.")));
}

#[test]
fn keys_are_unique_within_one_frame() {
    let mut state = AppState::new(Settings::default());
    state.stats.per_day = vec![bucket(today(), 2)];
    state.stats.total_faxes = 2;
    state.stats.top_senders = vec![("ACME".into(), 2)];
    let out = collect(&state);
    let mut seen = HashSet::new();
    for key in &out.keys {
        assert!(seen.insert(key.clone()), "duplicate key {key}");
    }
}

#[test]
fn view_builds_for_every_range_in_english() {
    let mut state = AppState::new(Settings::default());
    state.lang = Lang::En;
    for range in [
        StatsRange::Last7Days,
        StatsRange::Last30Days,
        StatsRange::Last365Days,
    ] {
        state.stats_range = range;
        assert!(collect(&state).keys.contains(&"chartcard".to_string()));
    }
}
