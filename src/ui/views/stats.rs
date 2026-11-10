//! Statistics page (UI_SPEC §6): range switch, metric cards, a bar chart built
//! from fixed-height coloured containers, and the top senders list.
//!
//! Chart buckets: 7 and 30 days are one bar per calendar day ending today
//! (zero-filled). The 365-day range is aggregated into the 12 calendar months
//! ending with the current month, so the chart never has more than 12 columns
//! of time for a year.

use super::kit::{self, Tone, px, truncate_units};
use crate::app::{AppState, Msg, StatsRange};
use crate::i18n::{Lang, t, tf};
use crate::model::DateBucket;
use acme_widgets::style::prelude::*;
use acme_widgets::style::{ColorToken, FontWeight, TextAlign};
use acme_widgets::{
    ButtonSize, ButtonVariant, CardVariant, WidgetNode, button, card, column, label_builder, row,
};
use chrono::{Datelike, Days, Local, NaiveDate};
use std::collections::BTreeMap;

/// Ranges longer than this many days are drawn as monthly buckets.
const DAILY_LIMIT: u32 = 60;
const MONTHS: u32 = 12;
/// Ranges up to this many days label every bar with its `MM-DD` date.
const SHORT_LABEL_MAX_DAYS: u32 = 7;
/// Fax counts are printed above the bars only when there are at most this many bars.
const COUNT_LABEL_MAX_BARS: usize = 12;
/// In the 30-day range every fifth axis label is printed (plus the last one).
const LABEL_EVERY: usize = 5;
const BARS_BOX_H: f32 = kit::CHART_H + 16.0;
/// Smallest visible bar height for a non-zero bucket.
const MIN_BAR_H: f32 = 2.0;
const TOP_LIMIT: usize = 10;
const TOP_NAME_UNITS: usize = 28;
const SENDER_BAR_MAX_W: f32 = 160.0;
const SENDER_BAR_MIN_W: f32 = 4.0;

/// Whole page body (right of the nav rail).
pub fn view(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let today = Local::now().date_naive();
    let range = state.stats_range;
    let header = kit::page_header(t(lang, "stats.title"), None, range_buttons(state));
    let body = column::<Msg>()
        .key("scroll.stats")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .overflow_scroll()
        .p(24.0)
        .gap(16.0)
        .child(metrics_row(state, range_days(range)))
        .child(chart_card(state, range, today))
        .child(top_card(state))
        .build();
    column::<Msg>()
        .key("page.stats")
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .child(header)
        .child(body)
        .build()
}

fn range_days(range: StatsRange) -> u32 {
    match range {
        StatsRange::Last7Days => 7,
        StatsRange::Last30Days => 30,
        StatsRange::Last365Days => 365,
    }
}

fn range_key(range: StatsRange) -> &'static str {
    match range {
        StatsRange::Last7Days => "btn.range.7d",
        StatsRange::Last30Days => "btn.range.30d",
        StatsRange::Last365Days => "btn.range.365d",
    }
}

fn range_label(lang: Lang, range: StatsRange) -> &'static str {
    match range {
        StatsRange::Last7Days => t(lang, "stats.range_7d"),
        StatsRange::Last30Days => t(lang, "stats.range_30d"),
        StatsRange::Last365Days => t(lang, "stats.range_365d"),
    }
}

fn range_buttons(state: &AppState) -> Vec<WidgetNode<Msg>> {
    [
        StatsRange::Last7Days,
        StatsRange::Last30Days,
        StatsRange::Last365Days,
    ]
    .into_iter()
    .map(|range| {
        let variant = if state.stats_range == range {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        button(range_key(range), range_label(state.lang, range))
            .variant(variant)
            .size(ButtonSize::Small)
            .on_click(Msg::SetStatsRange(range))
    })
    .collect()
}

fn metrics_row(state: &AppState, days: u32) -> WidgetNode<Msg> {
    let lang = state.lang;
    let stats = &state.stats;
    let total_faxes = stats.total_faxes.to_string();
    let total_pages = stats.total_pages.to_string();
    row::<Msg>()
        .key("metrics")
        .gap(12.0)
        .child(metric_card(t(lang, "stats.total_faxes"), &total_faxes))
        .child(metric_card(t(lang, "stats.total_pages"), &total_pages))
        .child(metric_card(
            t(lang, "stats.avg_per_day"),
            &avg_per_day_text(stats.total_faxes, days),
        ))
        .child(metric_card(
            t(lang, "stats.avg_pages"),
            &avg_pages_text(stats.total_pages, stats.total_faxes),
        ))
        .child(metric_card(
            t(lang, "stats.busiest_day"),
            &busiest_text(lang, &stats.per_day),
        ))
        .build()
}

fn metric_card(label: &str, value: &str) -> WidgetNode<Msg> {
    card::<Msg>()
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(4.0)
        .flex_1()
        .flex_shrink(1.0)
        .min_h(crate::ui::views::kit::px(0.0))
        .min_w(px(0.0))
        .child(kit::caption(label))
        .child(
            label_builder::<Msg>(value.to_string())
                .font_size(28.0)
                .font_weight(FontWeight::SemiBold)
                .build(),
        )
        .build()
}

fn chart_card(state: &AppState, range: StatsRange, today: NaiveDate) -> WidgetNode<Msg> {
    let lang = state.lang;
    let days = range_days(range);
    let monthly = days > DAILY_LIMIT;
    let buckets = bucket_counts(&state.stats.per_day, days, today);
    let peak = buckets.iter().map(|(_, count)| *count).max().unwrap_or(0);
    let title = if monthly {
        t(lang, "stats.chart_monthly")
    } else {
        t(lang, "stats.chart_daily")
    };
    let peak_text = peak.to_string();
    let peak_caption = tf(lang, "stats.chart_peak", &[("count", peak_text.as_str())]);
    let header = row::<Msg>()
        .gap(8.0)
        .child(kit::section(title))
        .child(kit::spacer())
        .child(kit::caption(&peak_caption))
        .build();
    card::<Msg>()
        .key("chartcard")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(12.0)
        .child(header)
        .child(chart(state, range, &buckets, today))
        .child(kit::caption(&range_caption(lang, &buckets, monthly)))
        .build()
}

/// Bars, count labels and the time axis. Every bar is a fixed-size container.
fn chart(
    state: &AppState,
    range: StatsRange,
    buckets: &[(NaiveDate, u64)],
    today: NaiveDate,
) -> WidgetNode<Msg> {
    let lang = state.lang;
    let days = range_days(range);
    let heights = bar_heights(&state.stats.per_day, days, today, kit::CHART_H);
    let (bar_w, gap) = bar_geometry(days);
    let show_count = buckets.len() <= COUNT_LABEL_MAX_BARS;
    let mut bars = row::<Msg>().gap(gap).child(kit::spacer());
    let mut axis = row::<Msg>().gap(gap).child(kit::spacer());
    for (index, ((date, count), (_, height))) in buckets.iter().zip(heights.iter()).enumerate() {
        bars = bars.child(bar_column(bar_w, *count, *height, show_count));
        let label = show_label(index, buckets.len(), days).then(|| bucket_label(*date, days));
        axis = axis.child(axis_column(bar_w, label));
    }
    bars = bars.child(kit::spacer());
    axis = axis.child(kit::spacer());
    let mut chart_column = column::<Msg>().gap(4.0).child(bars.build());
    if state.stats.total_faxes == 0 {
        let empty = row::<Msg>()
            .child(kit::spacer())
            .child(kit::caption(t(lang, "stats.chart_empty")))
            .child(kit::spacer())
            .build();
        chart_column = chart_column.child(empty);
    }
    chart_column
        .child(kit::hdivider())
        .child(axis.build())
        .build()
}

fn bar_column(bar_w: f32, count: u64, height: f32, show_count: bool) -> WidgetNode<Msg> {
    let mut column_builder = column::<Msg>()
        .w(px(bar_w))
        .h(px(BARS_BOX_H))
        .child(kit::spacer());
    if show_count && count > 0 {
        let count_text = count.to_string();
        column_builder = column_builder.child(
            label_builder::<Msg>(count_text)
                .font_size(12.0)
                .text_muted()
                .text_align(TextAlign::Center)
                .build(),
        );
    }
    let bar = if count == 0 {
        column::<Msg>()
            .w(px(bar_w))
            .h(px(MIN_BAR_H))
            .bg(ColorToken::Border)
            .rounded_sm()
    } else {
        column::<Msg>()
            .w(px(bar_w))
            .h(px(height))
            .bg(ColorToken::Primary)
            .rounded_sm()
    };
    column_builder.child(bar.build()).build()
}

fn axis_column(bar_w: f32, label: Option<String>) -> WidgetNode<Msg> {
    let mut column_builder = column::<Msg>().w(px(bar_w));
    if let Some(text) = label {
        column_builder = column_builder.child(
            label_builder::<Msg>(text)
                .font_size(12.0)
                .text_muted()
                .text_align(TextAlign::Center)
                .build(),
        );
    }
    column_builder.build()
}

/// `(bar width, gap)` in px for each range (UI_SPEC §6.1).
fn bar_geometry(days: u32) -> (f32, f32) {
    if days <= SHORT_LABEL_MAX_DAYS {
        (40.0, 12.0)
    } else if days <= DAILY_LIMIT {
        (16.0, 6.0)
    } else {
        (36.0, 12.0)
    }
}

/// Whether the axis label of bar `index` (of `len`) is printed.
fn show_label(index: usize, len: usize, days: u32) -> bool {
    days <= SHORT_LABEL_MAX_DAYS
        || days > DAILY_LIMIT
        || index.is_multiple_of(LABEL_EVERY)
        || index + 1 == len
}

fn bucket_label(date: NaiveDate, days: u32) -> String {
    let pattern = if days > DAILY_LIMIT {
        "%m"
    } else if days <= SHORT_LABEL_MAX_DAYS {
        "%m-%d"
    } else {
        "%d"
    };
    date.format(pattern).to_string()
}

fn range_caption(lang: Lang, buckets: &[(NaiveDate, u64)], monthly: bool) -> String {
    let (Some((first, _)), Some((last, _))) = (buckets.first(), buckets.last()) else {
        return String::new();
    };
    let pattern = if monthly { "%Y-%m" } else { "%Y-%m-%d" };
    let from = first.format(pattern).to_string();
    let to = last.format(pattern).to_string();
    tf(
        lang,
        "stats.chart_range",
        &[("from", from.as_str()), ("to", to.as_str())],
    )
}

fn top_card(state: &AppState) -> WidgetNode<Msg> {
    let lang = state.lang;
    let senders = &state.stats.top_senders;
    let max = senders.iter().map(|(_, count)| *count).max().unwrap_or(0);
    let mut card_builder = card::<Msg>()
        .key("topcard")
        .variant(CardVariant::Outlined)
        .padding(16.0)
        .gap(8.0)
        .child(kit::section(t(lang, "stats.top_senders")));
    if senders.is_empty() {
        card_builder = card_builder.child(kit::caption(t(lang, "stats.top_empty")));
    }
    for (index, (name, count)) in senders.iter().take(TOP_LIMIT).enumerate() {
        card_builder = card_builder.child(sender_row(lang, index, name, *count, max));
    }
    card_builder.build()
}

fn sender_row(lang: Lang, index: usize, name: &str, count: u64, max: u64) -> WidgetNode<Msg> {
    let rank = (index + 1).to_string();
    let count_text = count.to_string();
    let count_caption = tf(
        lang,
        "stats.sender_count",
        &[("count", count_text.as_str())],
    );
    let bar = column::<Msg>()
        .w(px(sender_bar_width(count, max)))
        .h(px(12.0))
        .bg(ColorToken::Primary)
        .rounded_sm()
        .build();
    row::<Msg>()
        .key(format!("top.{index}"))
        .h(px(28.0))
        .gap(12.0)
        .child(
            column::<Msg>()
                .w(px(24.0))
                .child(kit::caption(&rank))
                .build(),
        )
        .child(
            column::<Msg>()
                .w(px(220.0))
                .child(kit::body(&truncate_units(name, TOP_NAME_UNITS)))
                .build(),
        )
        .child(
            row::<Msg>()
                .w(px(SENDER_BAR_MAX_W))
                .h(px(12.0))
                .child(bar)
                .build(),
        )
        .child(
            column::<Msg>()
                .w(px(56.0))
                .child(kit::badge(&count_caption, Tone::Neutral))
                .build(),
        )
        .build()
}

/// Bar width for a sender: proportional to `max`, between 4 and 160 px.
fn sender_bar_width(count: u64, max: u64) -> f32 {
    let max = max.max(1);
    let width = (count as f32 / max as f32 * SENDER_BAR_MAX_W).round();
    width.clamp(SENDER_BAR_MIN_W, SENDER_BAR_MAX_W)
}

/// Fax counts per calendar day, summed when a date appears more than once.
fn totals_by_day(per_day: &[DateBucket]) -> BTreeMap<NaiveDate, u64> {
    per_day.iter().fold(BTreeMap::new(), |mut totals, bucket| {
        *totals.entry(bucket.date).or_insert(0) += bucket.count;
        totals
    })
}

/// Chart buckets for a range, oldest first: one per day for short ranges,
/// one per calendar month (12 months ending this month) for long ranges.
fn bucket_counts(
    per_day: &[DateBucket],
    range_days: u32,
    today: NaiveDate,
) -> Vec<(NaiveDate, u64)> {
    let totals = totals_by_day(per_day);
    if range_days > DAILY_LIMIT {
        monthly_counts(&totals, today)
    } else {
        daily_counts(&totals, range_days, today)
    }
}

fn daily_counts(
    totals: &BTreeMap<NaiveDate, u64>,
    days: u32,
    today: NaiveDate,
) -> Vec<(NaiveDate, u64)> {
    (0..days)
        .rev()
        .filter_map(|back| today.checked_sub_days(Days::new(u64::from(back))))
        .map(|date| (date, totals.get(&date).copied().unwrap_or(0)))
        .collect()
}

fn monthly_counts(totals: &BTreeMap<NaiveDate, u64>, today: NaiveDate) -> Vec<(NaiveDate, u64)> {
    let current = today.year() * 12 + today.month0() as i32;
    (0..MONTHS)
        .rev()
        .filter_map(|back| {
            let index = current - back as i32;
            let year = index.div_euclid(12);
            let month0 = index.rem_euclid(12) as u32;
            let first = NaiveDate::from_ymd_opt(year, month0 + 1, 1)?;
            let total = totals
                .iter()
                .filter(|(date, _)| date.year() == year && date.month0() == month0)
                .map(|(_, count)| *count)
                .sum::<u64>();
            Some((first, total))
        })
        .collect()
}

/// Scale counts to pixel heights: the largest count gets `max_h`, zero stays 0,
/// and any other count is at least [`MIN_BAR_H`] so it stays visible.
fn scale_heights(counts: &[u64], max_h: f32) -> Vec<f32> {
    let max = counts.iter().copied().max().unwrap_or(0).max(1);
    counts
        .iter()
        .map(|&count| {
            if count == 0 {
                0.0
            } else {
                (count as f32 / max as f32 * max_h).round().max(MIN_BAR_H)
            }
        })
        .collect()
}

/// One `(bucket date, bar height in px)` per chart bucket, oldest first.
/// `today` is passed in so the function stays pure and testable.
pub fn bar_heights(
    per_day: &[DateBucket],
    range_days: u32,
    today: NaiveDate,
    max_h: f32,
) -> Vec<(NaiveDate, f32)> {
    let buckets = bucket_counts(per_day, range_days, today);
    let counts: Vec<u64> = buckets.iter().map(|(_, count)| *count).collect();
    buckets
        .iter()
        .map(|(date, _)| *date)
        .zip(scale_heights(&counts, max_h))
        .collect()
}

fn avg_per_day_text(total_faxes: u64, days: u32) -> String {
    format!("{:.1}", total_faxes as f64 / f64::from(days.max(1)))
}

fn avg_pages_text(total_pages: u64, total_faxes: u64) -> String {
    if total_faxes == 0 {
        "—".to_string()
    } else {
        format!("{:.1}", total_pages as f64 / total_faxes as f64)
    }
}

/// Day with the most faxes; on a tie the earliest day wins. Zero days are ignored.
fn busiest_day(per_day: &[DateBucket]) -> Option<DateBucket> {
    per_day
        .iter()
        .copied()
        .filter(|bucket| bucket.count > 0)
        .max_by(|a, b| a.count.cmp(&b.count).then_with(|| b.date.cmp(&a.date)))
}

fn busiest_text(lang: Lang, per_day: &[DateBucket]) -> String {
    match busiest_day(per_day) {
        Some(bucket) => {
            let date = bucket.date.format("%Y-%m-%d").to_string();
            let count = bucket.count.to_string();
            tf(
                lang,
                "stats.busiest_value",
                &[("date", date.as_str()), ("count", count.as_str())],
            )
        }
        None => "—".to_string(),
    }
}

#[cfg(test)]
mod tests;
