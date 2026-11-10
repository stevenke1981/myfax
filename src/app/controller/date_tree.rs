//! Builds the year → month → day tree of the inbox date navigator.

use crate::app::state::{DayNode, MonthNode, YearNode};
use crate::model::DateBucket;
use chrono::Datelike;
use std::collections::{BTreeMap, HashMap};

/// Per-year nested counts: year → month → day → count.
type Counts = BTreeMap<i32, BTreeMap<u32, BTreeMap<u32, u64>>>;

/// Newest year first, newest month first, days ascending. A year is expanded when
/// the user chose so; otherwise only the current year is open.
pub(super) fn build(
    buckets: &[DateBucket],
    current_year: i32,
    overrides: &HashMap<i32, bool>,
) -> Vec<YearNode> {
    let mut counts: Counts = BTreeMap::new();
    for bucket in buckets {
        let days = counts
            .entry(bucket.date.year())
            .or_default()
            .entry(bucket.date.month())
            .or_default();
        *days.entry(bucket.date.day()).or_insert(0) += bucket.count;
    }
    counts
        .into_iter()
        .rev()
        .map(|(year, months)| year_node(year, months, current_year, overrides))
        .collect()
}

fn year_node(
    year: i32,
    months: BTreeMap<u32, BTreeMap<u32, u64>>,
    current_year: i32,
    overrides: &HashMap<i32, bool>,
) -> YearNode {
    let mut month_nodes: Vec<MonthNode> = months
        .into_iter()
        .map(|(month, days)| {
            let count = days.values().sum();
            let days = days
                .into_iter()
                .map(|(day, count)| DayNode { day, count })
                .collect();
            MonthNode { month, count, days }
        })
        .collect();
    month_nodes.reverse();
    YearNode {
        year,
        count: month_nodes.iter().map(|month| month.count).sum(),
        expanded: overrides
            .get(&year)
            .copied()
            .unwrap_or(year == current_year),
        months: month_nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn bucket(y: i32, m: u32, d: u32, count: u64) -> DateBucket {
        DateBucket {
            date: NaiveDate::from_ymd_opt(y, m, d).expect("valid test date"),
            count,
        }
    }

    #[test]
    fn orders_newest_first_and_sums_counts() {
        let buckets = [
            bucket(2025, 1, 2, 1),
            bucket(2025, 1, 1, 2),
            bucket(2026, 2, 5, 3),
            bucket(2026, 3, 1, 4),
            bucket(2026, 3, 9, 1),
        ];
        let tree = build(&buckets, 2026, &HashMap::new());
        let years: Vec<i32> = tree.iter().map(|year| year.year).collect();
        assert_eq!(years, vec![2026, 2025]);
        assert_eq!(tree[0].count, 8);
        let months: Vec<u32> = tree[0].months.iter().map(|m| m.month).collect();
        assert_eq!(months, vec![3, 2]);
        assert_eq!(tree[0].months[0].count, 5);
        let days: Vec<u32> = tree[0].months[0].days.iter().map(|d| d.day).collect();
        assert_eq!(days, vec![1, 9]);
        assert_eq!(tree[1].months[0].days[0].count, 2);
    }

    #[test]
    fn current_year_is_expanded_unless_overridden() {
        let buckets = [bucket(2025, 1, 1, 1), bucket(2026, 1, 1, 1)];
        let mut overrides = HashMap::new();
        let tree = build(&buckets, 2026, &overrides);
        assert!(tree[0].expanded);
        assert!(!tree[1].expanded);
        overrides.insert(2026, false);
        overrides.insert(2025, true);
        let tree = build(&buckets, 2026, &overrides);
        assert!(!tree[0].expanded);
        assert!(tree[1].expanded);
    }

    #[test]
    fn empty_input_gives_empty_tree() {
        assert!(build(&[], 2026, &HashMap::new()).is_empty());
    }
}
