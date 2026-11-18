//! Dates PDF (`D:AAAAMMJJHHmmSS±HH'mm'`, ISO 32000 § 7.9.4), sans
//! dépendance : conversion depuis et vers les secondes depuis 1970 (UTC).

/// `D:AAAAMMJJHHmmSSZ` pour cet instant UTC.
pub(crate) fn format(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let seconds = unix % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "D:{year:04}{month:02}{day:02}{:02}{:02}{:02}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Instant UTC d'une date PDF ; champs absents = valeurs par défaut de la norme.
pub(crate) fn parse(text: &str) -> Option<u64> {
    let text = text.strip_prefix("D:").unwrap_or(text);
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 4 {
        return None;
    }
    let field = |from: usize, len: usize, default: u32| -> Option<u32> {
        match digits.get(from..from + len) {
            Some(part) => part.parse().ok(),
            None => Some(default),
        }
    };
    let year = field(0, 4, 0)? as i64;
    let month = field(4, 2, 1)?;
    let day = field(6, 2, 1)?;
    let hour = field(8, 2, 0)?;
    let minute = field(10, 2, 0)?;
    let second = field(12, 2, 0)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let mut unix = days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3600
        + i64::from(minute) * 60
        + i64::from(second.min(59));
    // Décalage : « +01'00' » = heure locale en avance sur UTC.
    let rest = &text[digits.len()..];
    let mut chars = rest.chars();
    if let Some(sign @ ('+' | '-')) = chars.next() {
        let offset: String = chars.filter(char::is_ascii_digit).collect();
        let hours: i64 = offset.get(0..2).and_then(|h| h.parse().ok()).unwrap_or(0);
        let minutes: i64 = offset.get(2..4).and_then(|m| m.parse().ok()).unwrap_or(0);
        let shift = hours * 3600 + minutes * 60;
        unix += if sign == '+' { -shift } else { shift };
    }
    u64::try_from(unix).ok()
}

/// Jours depuis 1970 → (année, mois, jour), calendrier grégorien (H. Hinnant).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// (année, mois, jour) → jours depuis 1970.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let month = i64::from(month);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(format(0), "D:19700101000000Z");
        // 18/11/2026 12:00:00 UTC.
        assert_eq!(format(1_795_003_200), "D:20261118120000Z");
        // 29 février d'une année bissextile.
        assert_eq!(format(1_709_208_000), "D:20240229120000Z");
    }

    #[test]
    fn parses_what_it_formats() {
        for unix in [0, 1_709_208_000, 1_795_003_200, 4_102_444_799] {
            assert_eq!(parse(&format(unix)), Some(unix));
        }
    }

    #[test]
    fn applies_time_zone_offsets() {
        // 13:00 à Paris (UTC+1) = 12:00 UTC.
        assert_eq!(parse("D:20261118130000+01'00'"), Some(1_795_003_200));
        assert_eq!(parse("D:20261118070000-05'00"), Some(1_795_003_200));
    }

    #[test]
    fn missing_fields_take_defaults_and_garbage_is_refused() {
        assert_eq!(parse("D:2026"), parse("D:20260101000000Z"));
        assert_eq!(parse("D:20261340"), None);
        assert_eq!(parse("hier"), None);
    }
}
