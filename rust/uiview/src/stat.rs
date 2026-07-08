//! Shared StatPanel computation — the ONE Rust implementation of a KPI tile's
//! delta / trend / formatting, mirroring the TypeScript `computeStat`
//! (@savvifi/meridian-schemas/uiview stat.ts) EXACTLY so the tui and the web
//! renderers never diverge. A parity test in each language asserts identical
//! output for identical input.
//!
//! The delta/trend is COMPUTED from the data (previous / series), never trusted
//! from an author-marked direction. Semantic good/bad color applies ONLY when
//! `higher_is_better` is explicitly set. Number formatting is deterministic
//! integer math, byte-identical to the TS formatter.

use crate::proto::StatPanel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatTrend {
    Up,
    Down,
    Flat,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatSemantics {
    Good,
    Bad,
    Neutral,
}

/// The computed view of a StatPanel: formatted value, trend, formatted delta,
/// semantic color, and the sparkline series.
#[derive(Debug, Clone, PartialEq)]
pub struct StatComputed {
    pub formatted_value: String,
    pub trend: StatTrend,
    pub formatted_delta: Option<String>,
    pub semantics: StatSemantics,
    pub series: Vec<f64>,
}

// Decompose |round(n, 2dp)| into (negative, integer part, frac 0..99).
fn parts(n: f64) -> (bool, u64, u64) {
    let scaled = ((n.abs() * 100.0) + 1e-9).round() as i64 * if n < 0.0 { -1 } else { 1 };
    let neg = scaled < 0;
    let a = scaled.unsigned_abs();
    (neg, a / 100, a % 100)
}

fn group(int: u64) -> String {
    let s = int.to_string();
    let mut out = String::new();
    let len = s.len();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn trim_num(n: f64, grouped: bool) -> String {
    let (neg, int, frac) = parts(n);
    let mut s = if grouped { group(int) } else { int.to_string() };
    if frac != 0 {
        let mut f = format!("{frac:02}");
        while f.ends_with('0') {
            f.pop();
        }
        if f.is_empty() {
            f.push('0');
        }
        s = format!("{s}.{f}");
    }
    if neg && !(int == 0 && frac == 0) {
        format!("-{s}")
    } else {
        s
    }
}

fn currency(n: f64) -> String {
    let (neg, int, frac) = parts(n);
    let s = format!("${}.{:02}", group(int), frac);
    if neg && !(int == 0 && frac == 0) {
        format!("-{s}")
    } else {
        s
    }
}

fn compact(n: f64) -> String {
    let a = n.abs();
    for (div, suffix) in [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")] {
        if a >= div {
            return format!("{}{}", trim_num(n / div, false), suffix);
        }
    }
    trim_num(n, false)
}

/// Format a raw number per a ValueFormat enum value (0/1 number, 2 percent,
/// 3 currency, 4 compact, 5 plain). Deterministic.
pub fn format_stat_number(n: f64, format: i32) -> String {
    match format {
        2 => format!("{}%", trim_num(n, false)),
        3 => currency(n),
        4 => compact(n),
        5 => trim_num(n, false),
        _ => trim_num(n, true),
    }
}

/// The arrow glyph for a trend (shared so web + tui match).
pub fn trend_arrow(trend: StatTrend) -> &'static str {
    match trend {
        StatTrend::Up => "↑",
        StatTrend::Down => "↓",
        StatTrend::Flat => "→",
        StatTrend::None => "",
    }
}

fn map_trend(override_val: i32) -> StatTrend {
    match override_val {
        1 => StatTrend::Up,
        2 => StatTrend::Down,
        3 => StatTrend::Flat,
        _ => StatTrend::None,
    }
}

/// Compute a StatPanel's value/delta/trend/semantics — the parity-critical core.
pub fn compute_stat(panel: &StatPanel) -> StatComputed {
    let unit_suffix = if !panel.unit.is_empty() && panel.format != 2 && panel.format != 3 {
        format!(" {}", panel.unit)
    } else {
        String::new()
    };
    let formatted_value = format!(
        "{}{}",
        format_stat_number(panel.value, panel.format),
        unit_suffix
    );

    // Raw delta: value − previous, else last − first of series.
    let raw: Option<f64> = if let Some(p) = panel.previous {
        Some(panel.value - p)
    } else if panel.series.len() >= 2 {
        Some(panel.series[panel.series.len() - 1] - panel.series[0])
    } else {
        None
    };

    let computed_trend = match raw {
        None => StatTrend::None,
        Some(r) if r > 0.0 => StatTrend::Up,
        Some(r) if r < 0.0 => StatTrend::Down,
        Some(_) => StatTrend::Flat,
    };
    let trend = if panel.trend_override != 0 {
        map_trend(panel.trend_override)
    } else {
        computed_trend
    };

    let formatted_delta = if let Some(d) = &panel.delta_override {
        Some(d.clone())
    } else {
        raw.map(|r| {
            format!(
                "{}{}",
                if r >= 0.0 { "+" } else { "-" },
                format_stat_number(r.abs(), panel.format)
            )
        })
    };

    let semantics = match panel.higher_is_better {
        Some(hib) if trend == StatTrend::Up || trend == StatTrend::Down => {
            if (trend == StatTrend::Up) == hib {
                StatSemantics::Good
            } else {
                StatSemantics::Bad
            }
        }
        _ => StatSemantics::Neutral,
    };

    let series = if panel.series.len() >= 2 {
        panel.series.clone()
    } else {
        Vec::new()
    };

    StatComputed {
        formatted_value,
        trend,
        formatted_delta,
        semantics,
        series,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::StatPanel;

    fn stat(value: f64, format: i32) -> StatPanel {
        StatPanel {
            label: "m".into(),
            value,
            format,
            unit: String::new(),
            previous: None,
            series: vec![],
            delta_override: None,
            trend_override: 0,
            higher_is_better: None,
            caption: String::new(),
        }
    }

    // ── PARITY VECTORS — these exact (input → expected) rows are duplicated in
    //    the TypeScript parity test (meridian-web-react tests/stat_parity.test.ts).
    //    Both languages must produce identical strings/trends. ──────────────────

    #[test]
    fn format_parity_vectors() {
        assert_eq!(format_stat_number(1234.5, 1), "1,234.5"); // NUMBER grouped
        assert_eq!(format_stat_number(1234.0, 5), "1234"); // PLAIN
        assert_eq!(format_stat_number(87.5, 2), "87.5%"); // PERCENT
        assert_eq!(format_stat_number(1234.0, 3), "$1,234.00"); // CURRENCY
        assert_eq!(format_stat_number(1500000.0, 4), "1.5M"); // COMPACT
        assert_eq!(format_stat_number(1200.0, 4), "1.2K");
        assert_eq!(format_stat_number(-5.0, 3), "-$5.00");
        assert_eq!(format_stat_number(12.567, 1), "12.57"); // rounds to 2dp
    }

    #[test]
    fn computes_delta_from_previous_with_semantic_color() {
        // 120 vs previous 150, higher_is_better → a decline is BAD.
        let mut p = stat(120.0, 1);
        p.previous = Some(150.0);
        p.higher_is_better = Some(true);
        let c = compute_stat(&p);
        assert_eq!(c.formatted_value, "120");
        assert_eq!(c.trend, StatTrend::Down);
        assert_eq!(c.formatted_delta.as_deref(), Some("-30"));
        assert_eq!(c.semantics, StatSemantics::Bad);
    }

    #[test]
    fn computes_trend_from_series_catching_a_declining_mismark() {
        // A declining series that a bad dashboard would mark "up" — computation
        // says DOWN. No higher_is_better → neutral (honest).
        let mut p = stat(5.0, 5);
        p.series = vec![10.0, 8.0, 6.0, 5.0];
        let c = compute_stat(&p);
        assert_eq!(c.trend, StatTrend::Down);
        assert_eq!(c.formatted_delta.as_deref(), Some("-5")); // 5 − 10
        assert_eq!(c.semantics, StatSemantics::Neutral);
        assert_eq!(c.series.len(), 4); // sparkline available
    }

    #[test]
    fn no_semantic_color_without_higher_is_better() {
        let mut p = stat(200.0, 1);
        p.previous = Some(150.0);
        let c = compute_stat(&p);
        assert_eq!(c.trend, StatTrend::Up);
        assert_eq!(c.semantics, StatSemantics::Neutral); // up ≠ good unless declared
    }
}
