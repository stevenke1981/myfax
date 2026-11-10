//! i18n fragment `stats` — rows are (key, zh-TW, en). Owner: see docs/team/myfax/BRIEF.md.

pub const ENTRIES: &[(&str, &str, &str)] = &[
    ("stats.title", "統計", "Statistics"),
    ("stats.range_7d", "近 7 天", "7 days"),
    ("stats.range_30d", "近 30 天", "30 days"),
    ("stats.range_365d", "近 1 年", "1 year"),
    ("stats.total_faxes", "傳真總數", "Total faxes"),
    ("stats.total_pages", "總頁數", "Total pages"),
    ("stats.avg_per_day", "日均傳真數", "Avg faxes per day"),
    ("stats.avg_pages", "平均每份頁數", "Avg pages per fax"),
    ("stats.busiest_day", "收件最多的一天", "Busiest day"),
    (
        "stats.busiest_value",
        "{date}（{count} 份）",
        "{date} ({count})",
    ),
    ("stats.chart_daily", "每日傳真數量", "Faxes per day"),
    ("stats.chart_monthly", "每月傳真數量", "Faxes per month"),
    ("stats.chart_peak", "最高：{count}", "Peak: {count}"),
    (
        "stats.chart_empty",
        "此期間沒有傳真",
        "No faxes in this period",
    ),
    ("stats.chart_range", "{from} 至 {to}", "{from} to {to}"),
    ("stats.top_senders", "主要寄件者", "Top senders"),
    (
        "stats.top_empty",
        "此期間沒有資料",
        "No data for this period",
    ),
    ("stats.sender_count", "{count} 份", "{count}"),
];
