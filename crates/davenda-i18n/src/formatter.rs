use crate::{CurrencyCode, LocaleTag, TimeZoneId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory {
    One,
    Other,
}

pub struct Formatter;

impl Formatter {
    pub fn plural_category(locale: &LocaleTag, value: i64) -> PluralCategory {
        match locale.as_str() {
            "fr-FR" => {
                if matches!(value, 0 | 1) {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
            _ => {
                if value == 1 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
        }
    }

    pub fn format_number(locale: &LocaleTag, value: i64) -> String {
        let negative = value < 0;
        let mut digits = value.abs().to_string();
        let separator = if locale.as_str() == "fr-FR" { ' ' } else { ',' };
        let mut groups = Vec::new();
        while digits.len() > 3 {
            let remainder = digits.split_off(digits.len() - 3);
            groups.push(remainder);
        }
        groups.push(digits);
        groups.reverse();
        let rendered = groups.join(&separator.to_string());
        if negative {
            format!("-{rendered}")
        } else {
            rendered
        }
    }

    pub fn format_money(locale: &LocaleTag, currency: &CurrencyCode, minor_units: i64) -> String {
        let major = minor_units / 100;
        let cents = minor_units.abs() % 100;
        let number = Self::format_number(locale, major);
        let decimal_separator = if locale.as_str() == "fr-FR" { ',' } else { '.' };
        match locale.as_str() {
            "fr-FR" => format!("{number}{decimal_separator}{cents:02} {currency}"),
            _ => format!("{currency} {number}{decimal_separator}{cents:02}"),
        }
    }

    pub fn format_datetime(locale: &LocaleTag, unix_seconds: i64, timezone: &TimeZoneId) -> String {
        let label = match locale.as_str() {
            "fr-FR" => "heure locale",
            _ => "local time",
        };
        format!("{unix_seconds} ({timezone}, {label})")
    }
}
