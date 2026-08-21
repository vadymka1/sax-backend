use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct SpaSection {
    pub id: Uuid,
    pub page_id: Uuid,
    pub section_key: String,
    pub title: String,
    pub navigation_label: String,
    pub sort_order: i32,
    pub is_visible: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn validate_spa_section_key(key: &str) -> bool {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return false;
    }
    // Grammar: lowercase alphanumeric segments separated by exactly one hyphen (^[a-z0-9]+(?:-[a-z0-9]+)*$)
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.is_empty() {
        return false;
    }
    for part in parts {
        if part.is_empty()
            || !part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

pub fn generate_slug(title: &str) -> String {
    let mut result = String::new();
    let trimmed = title.trim();

    for c in trimmed.chars() {
        let lower = c.to_lowercase();
        for lc in lower {
            match lc {
                'a'..='z' | '0'..='9' => result.push(lc),
                // Accent/diacritic mapping
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' => {
                    result.push('a')
                }
                'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => result.push('c'),
                'ď' | 'đ' => result.push('d'),
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => result.push('e'),
                'ĝ' | 'ğ' | 'ġ' | 'ģ' => result.push('g'),
                'ĥ' | 'ħ' => result.push('h'),
                'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ǐ' => result.push('i'),
                'ĵ' => result.push('j'),
                'ķ' => result.push('k'),
                'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => result.push('l'),
                'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' => result.push('n'),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ǒ' => {
                    result.push('o')
                }
                'ŕ' | 'ŗ' | 'ř' => result.push('r'),
                'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => result.push('s'),
                'ť' | 'ţ' | 'ț' => result.push('t'),
                'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ǔ' => {
                    result.push('u')
                }
                'ŵ' => result.push('w'),
                'ý' | 'ÿ' | 'ŷ' => result.push('y'),
                'ź' | 'ż' | 'ž' => result.push('z'),
                'æ' => {
                    result.push('a');
                    result.push('e');
                }
                'œ' => {
                    result.push('o');
                    result.push('e');
                }
                'ß' => {
                    result.push('s');
                    result.push('s');
                }
                _ => {
                    if !result.ends_with('-') && !result.is_empty() {
                        result.push('-');
                    }
                }
            }
        }
    }

    let slug = result.trim_matches('-').to_string();
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_dash {
                collapsed.push(ch);
                prev_dash = true;
            }
        } else {
            collapsed.push(ch);
            prev_dash = false;
        }
    }

    let final_slug = collapsed.trim_matches('-').to_string();
    if final_slug.is_empty() {
        "section".to_string()
    } else {
        final_slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spa_section_key_validation() {
        // Valid section keys
        assert!(validate_spa_section_key("about"));
        assert!(validate_spa_section_key("about-us"));
        assert!(validate_spa_section_key("festival-2026"));
        assert!(validate_spa_section_key("works2"));
        assert!(validate_spa_section_key("section-123"));

        // Invalid section keys
        assert!(!validate_spa_section_key("-"));
        assert!(!validate_spa_section_key("---"));
        assert!(!validate_spa_section_key("-about"));
        assert!(!validate_spa_section_key("about-"));
        assert!(!validate_spa_section_key("about--us"));
        assert!(!validate_spa_section_key("About-us"));
        assert!(!validate_spa_section_key("about_us"));
        assert!(!validate_spa_section_key("about us"));
        assert!(!validate_spa_section_key("/about"));
        assert!(!validate_spa_section_key(""));
    }

    #[test]
    fn test_generate_slug_variants() {
        let cases = vec![
            ("Partners", "partners"),
            ("Our Amazing Team", "our-amazing-team"),
            ("Festival 2027", "festival-2027"),
            ("Press & Media", "press-media"),
            ("Český Festival", "cesky-festival"),
            ("Můj Tým", "muj-tym"),
            ("Über Uns", "uber-uns"),
            ("  Leading & Trailing Spaces  ", "leading-trailing-spaces"),
            ("Multiple---Hyphens &   Spaces!", "multiple-hyphens-spaces"),
            ("!!!", "section"),
        ];

        for (input, expected) in cases {
            let slug = generate_slug(input);
            assert_eq!(
                slug, expected,
                "Failed slug generation for input '{}'",
                input
            );
            assert!(
                validate_spa_section_key(&slug),
                "Generated slug '{}' must pass validate_spa_section_key",
                slug
            );
        }
    }
}
