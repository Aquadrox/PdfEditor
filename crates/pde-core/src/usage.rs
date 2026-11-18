//! Consommation de l'IA (IA-03) : jetons et coût par jour, totaux du jour,
//! du mois et depuis le début. Jamais de texte : seulement des compteurs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ai::Usage;
use crate::settings::Pricing;

impl Pricing {
    /// Coût d'une requête, dans la devise du tarif.
    pub fn cost(&self, usage: &Usage) -> f64 {
        let cached = usage.cached_input_tokens.min(usage.input_tokens);
        let uncached = usage.input_tokens - cached;
        (uncached as f64 * self.input_per_million
            + cached as f64 * self.cached_input_per_million
            + usage.output_tokens as f64 * self.output_per_million)
            / 1_000_000.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Totals {
    pub requests: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

impl Totals {
    fn add(&mut self, other: &Totals) {
        self.requests += other.requests;
        self.input_tokens += other.input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.output_tokens += other.output_tokens;
        self.cost += other.cost;
    }
}

/// Totaux par jour (`AAAA-MM-JJ`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageLog {
    pub days: BTreeMap<String, Totals>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct UsageSummary {
    pub today: Totals,
    pub month: Totals,
    pub total: Totals,
}

impl UsageLog {
    /// Ajoute une requête du jour `day` (`AAAA-MM-JJ`).
    pub fn record(&mut self, day: &str, usage: &Usage, pricing: &Pricing) {
        let entry = self.days.entry(day.to_owned()).or_default();
        entry.add(&Totals {
            requests: 1,
            input_tokens: usage.input_tokens,
            cached_input_tokens: usage.cached_input_tokens,
            output_tokens: usage.output_tokens,
            cost: pricing.cost(usage),
        });
    }

    /// Totaux du jour `today` (`AAAA-MM-JJ`), de son mois et de toujours.
    pub fn summary(&self, today: &str) -> UsageSummary {
        let month = today.get(..7).unwrap_or(today);
        let mut summary = UsageSummary::default();
        for (day, totals) in &self.days {
            summary.total.add(totals);
            if day.starts_with(month) {
                summary.month.add(totals);
            }
            if day == today {
                summary.today.add(totals);
            }
        }
        summary
    }
}

/// `AAAA-MM-JJ` (UTC) d'un instant en secondes depuis 1970.
pub fn day_of(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    // Calendrier grégorien (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing() -> Pricing {
        Pricing {
            input_per_million: 1.0,
            cached_input_per_million: 0.1,
            output_per_million: 2.0,
            currency: "EUR".into(),
        }
    }

    #[test]
    fn cost_counts_cached_tokens_at_their_price() {
        let usage = Usage {
            input_tokens: 1_000_000,
            cached_input_tokens: 500_000,
            output_tokens: 250_000,
        };
        assert!((pricing().cost(&usage) - (0.5 + 0.05 + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn totals_by_day_month_and_ever() {
        let mut log = UsageLog::default();
        let usage = Usage {
            input_tokens: 1000,
            cached_input_tokens: 0,
            output_tokens: 100,
        };
        log.record("2026-10-31", &usage, &pricing());
        log.record("2026-11-17", &usage, &pricing());
        log.record("2026-11-18", &usage, &pricing());
        log.record("2026-11-18", &usage, &pricing());
        let summary = log.summary("2026-11-18");
        assert_eq!(summary.today.requests, 2);
        assert_eq!(summary.month.requests, 3);
        assert_eq!(summary.total.requests, 4);
        assert_eq!(summary.total.output_tokens, 400);
    }

    #[test]
    fn days_are_dated_in_utc() {
        assert_eq!(day_of(0), "1970-01-01");
        assert_eq!(day_of(1_795_003_200), "2026-11-18");
        assert_eq!(day_of(1_709_208_000), "2024-02-29");
    }
}
