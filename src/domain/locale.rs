use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

pub const DEFAULT_LOCALE: Locale = Locale::En;
pub const DEFAULT_LOCALE_STR: &str = "en";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    En,
    De,
}

impl Locale {
    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::De => "de",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "en" => Ok(Locale::En),
            "de" => Ok(Locale::De),
            other => Err(format!(
                "Invalid locale '{}'. Supported locales are: 'en', 'de'",
                other
            )),
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        DEFAULT_LOCALE
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Locale {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_parse_and_display() {
        assert_eq!(Locale::parse("en"), Ok(Locale::En));
        assert_eq!(Locale::parse("de"), Ok(Locale::De));
        assert!(Locale::parse("fr").is_err());
        assert!(Locale::parse("EN").is_err());
        assert!(Locale::parse("de-DE").is_err());
        assert!(Locale::parse("").is_err());

        assert_eq!(Locale::En.as_str(), "en");
        assert_eq!(Locale::De.as_str(), "de");
        assert_eq!(format!("{}", Locale::En), "en");
        assert_eq!(format!("{}", Locale::De), "de");
        assert_eq!(Locale::default(), Locale::En);
    }
}
