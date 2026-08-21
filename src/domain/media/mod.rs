use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    Video,
    Youtube,
    ExternalVideo,
    Document,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::Image => "image",
            MediaType::Video => "video",
            MediaType::Youtube => "youtube",
            MediaType::ExternalVideo => "external_video",
            MediaType::Document => "document",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "image" => Some(MediaType::Image),
            "video" => Some(MediaType::Video),
            "youtube" => Some(MediaType::Youtube),
            "external_video" => Some(MediaType::ExternalVideo),
            "document" => Some(MediaType::Document),
            _ => None,
        }
    }
}

pub struct YoutubeUrlParser;

impl YoutubeUrlParser {
    pub fn parse_id(url_str: &str) -> Option<String> {
        let trimmed = url_str.trim();
        if trimmed.is_empty() || trimmed.contains("<iframe") || trimmed.contains("html") {
            return None;
        }

        let parsed = Url::parse(trimmed).ok()?;
        if parsed.scheme() != "https" {
            return None;
        }

        let host = parsed.host_str()?;
        let is_valid_host = matches!(
            host,
            "youtube.com" | "www.youtube.com" | "m.youtube.com" | "youtu.be"
        );
        if !is_valid_host {
            return None;
        }

        let extracted_id: Option<String> = if host == "youtu.be" {
            let path = parsed.path().trim_start_matches('/');
            path.split('/').next().map(|s| s.to_string())
        } else if parsed.path() == "/watch" {
            parsed
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.to_string())
        } else if let Some(stripped) = parsed.path().strip_prefix("/embed/") {
            stripped.split('/').next().map(|s| s.to_string())
        } else if let Some(stripped) = parsed.path().strip_prefix("/shorts/") {
            stripped.split('/').next().map(|s| s.to_string())
        } else {
            None
        };

        let id = extracted_id?;
        let id_trimmed = id.trim();
        if Self::is_valid_id(id_trimmed) {
            Some(id_trimmed.to_string())
        } else {
            None
        }
    }

    pub fn is_valid_id(id: &str) -> bool {
        if id.len() != 11 {
            return false;
        }
        id.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    pub fn build_canonical_url(video_id: &str) -> String {
        format!("https://www.youtube.com/watch?v={}", video_id)
    }

    pub fn build_embed_url(video_id: &str) -> String {
        format!("https://www.youtube.com/embed/{}", video_id)
    }

    pub fn build_thumbnail_url(video_id: &str) -> String {
        format!("https://img.youtube.com/vi/{}/hqdefault.jpg", video_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_parser_valid_urls() {
        let urls = vec![
            ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://youtu.be/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/embed/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://www.youtube.com/shorts/dQw4w9WgXcQ", "dQw4w9WgXcQ"),
            ("https://m.youtube.com/watch?v=dQw4w9WgXcQ", "dQw4w9WgXcQ"),
        ];

        for (url, expected) in urls {
            assert_eq!(YoutubeUrlParser::parse_id(url), Some(expected.to_string()));
        }
    }

    #[test]
    fn test_youtube_parser_rejections() {
        let invalid_urls = vec![
            "http://youtube.com/watch?v=dQw4w9WgXcQ", // non-HTTPS
            "https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ", // fake subdomain
            "https://evil.example/youtu.be/dQw4w9WgXcQ", // embedded host in path
            "https://www.youtube.com/watch?v=short_id", // invalid length
            "https://www.youtube.com/watch?v=invalid!id!!", // invalid chars
            "https://www.youtube.com/watch",          // missing v query param
            "<iframe src=\"https://www.youtube.com/embed/dQw4w9WgXcQ\"></iframe>", // raw iframe
        ];

        for url in invalid_urls {
            assert_eq!(
                YoutubeUrlParser::parse_id(url),
                None,
                "Failed for URL: {}",
                url
            );
        }
    }
}
