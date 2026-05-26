mod facebook;
mod tiktok;
mod youtube;

use std::time::Duration;

#[cfg(test)]
use std::{collections::HashMap, sync::Arc};

use reqwest::Client;

pub use facebook::parse_facebook;
pub use tiktok::parse_tiktok;
pub use youtube::parse_youtube;

use crate::model::VideoInfo;

#[derive(Debug)]
pub enum ExtractError {
    UnsupportedUrl,
    FetchFailed(String),
    MissingJson(&'static str),
    InvalidJson(String),
    MissingField(&'static str),
    NoStreams,
}

impl ExtractError {
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedUrl => {
                "unsupported url; expected YouTube, TikTok, or Facebook".to_string()
            }
            Self::FetchFailed(error) => format!("failed to fetch source html: {error}"),
            Self::MissingJson(name) => format!("could not find {name} json in html"),
            Self::InvalidJson(error) => format!("invalid provider json: {error}"),
            Self::MissingField(field) => format!("missing provider field: {field}"),
            Self::NoStreams => "no playable streams found".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    YouTube,
    TikTok,
    Facebook,
}

#[derive(Clone)]
pub struct Extractor {
    fetcher: HtmlFetcher,
}

#[derive(Clone)]
enum HtmlFetcher {
    Live(Client),
    #[cfg(test)]
    Fixture(Arc<HashMap<String, String>>),
}

impl Extractor {
    pub fn live() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36")
            .build()?;

        Ok(Self {
            fetcher: HtmlFetcher::Live(client),
        })
    }

    #[cfg(test)]
    pub fn fixture(fixtures: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            fetcher: HtmlFetcher::Fixture(Arc::new(fixtures.into_iter().collect())),
        }
    }

    #[allow(dead_code)]
    pub async fn extract(&self, source_url: &str) -> Result<VideoInfo, ExtractError> {
        self.extract_with_cookie(source_url, None).await
    }

    pub async fn extract_with_cookie(
        &self,
        source_url: &str,
        cookie: Option<&str>,
    ) -> Result<VideoInfo, ExtractError> {
        let provider = provider_for_url(source_url).ok_or(ExtractError::UnsupportedUrl)?;
        let html = self.fetch_html(source_url, cookie).await?;

        match provider {
            Provider::YouTube => parse_youtube(source_url, &html),
            Provider::TikTok => parse_tiktok(source_url, &html),
            Provider::Facebook => parse_facebook(source_url, &html),
        }
    }

    async fn fetch_html(
        &self,
        source_url: &str,
        cookie: Option<&str>,
    ) -> Result<String, ExtractError> {
        match &self.fetcher {
            HtmlFetcher::Live(client) => {
                let mut request = client.get(source_url);

                if let Some(cookie) = clean_cookie(cookie) {
                    request = request.header(reqwest::header::COOKIE, cookie);
                }

                let response = request
                    .send()
                    .await
                    .map_err(|error| ExtractError::FetchFailed(error.to_string()))?;
                let status = response.status();

                if !status.is_success() {
                    return Err(ExtractError::FetchFailed(format!(
                        "provider returned status {status}"
                    )));
                }

                response
                    .text()
                    .await
                    .map_err(|error| ExtractError::FetchFailed(error.to_string()))
            }
            #[cfg(test)]
            HtmlFetcher::Fixture(fixtures) => fixtures
                .get(source_url)
                .cloned()
                .ok_or_else(|| ExtractError::FetchFailed("fixture not found".to_string())),
        }
    }
}

fn provider_for_url(source_url: &str) -> Option<Provider> {
    let host = host_from_url(source_url)?;

    if host == "youtu.be" || host.ends_with(".youtube.com") || host == "youtube.com" {
        return Some(Provider::YouTube);
    }

    if host == "tiktok.com" || host.ends_with(".tiktok.com") {
        return Some(Provider::TikTok);
    }

    if host == "facebook.com"
        || host.ends_with(".facebook.com")
        || host == "fb.watch"
        || host.ends_with(".fb.watch")
    {
        return Some(Provider::Facebook);
    }

    None
}

fn host_from_url(source_url: &str) -> Option<String> {
    let without_scheme = source_url
        .strip_prefix("https://")
        .or_else(|| source_url.strip_prefix("http://"))?;
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .filter(|part| !part.is_empty())?;
    let host_port = authority.rsplit('@').next()?;
    let host = host_port.split(':').next()?.trim().to_ascii_lowercase();

    if host.is_empty() {
        return None;
    }

    Some(host)
}

fn collect_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;

    for segment in path {
        current = current.get(*segment)?;
    }

    current.as_str().map(ToOwned::to_owned)
}

fn collect_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut current = value;

    for segment in path {
        current = current.get(*segment)?;
    }

    current
        .as_u64()
        .or_else(|| current.as_str().and_then(|raw| raw.parse().ok()))
}

fn last_thumbnail_url(value: &serde_json::Value) -> Option<String> {
    let thumbnails = value
        .get("thumbnail")
        .and_then(|thumbnail| thumbnail.get("thumbnails"))
        .and_then(|items| items.as_array())?;

    thumbnails
        .iter()
        .rev()
        .find_map(|item| item.get("url").and_then(|url| url.as_str()))
        .map(ToOwned::to_owned)
}

fn first_string(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .as_array()
        .and_then(|items| items.iter().find_map(first_string))
}

fn clean_cookie(cookie: Option<&str>) -> Option<&str> {
    cookie.map(str::trim).filter(|value| !value.is_empty())
}

fn extract_json_object_after(html: &str, start_index: usize) -> Option<&str> {
    let bytes = html.as_bytes();
    let mut object_start = None;

    for (index, byte) in bytes.iter().enumerate().skip(start_index) {
        if *byte == b'{' {
            object_start = Some(index);
            break;
        }
    }

    let object_start = object_start?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate().skip(object_start) {
        let character = *byte as char;

        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return html.get(object_start..=index);
                }
            }
            _ => {}
        }
    }

    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &input[index + 1..index + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    decoded.push(value);
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn query_param(input: &str, key: &str) -> Option<String> {
    input.split('&').find_map(|part| {
        let (part_key, value) = part.split_once('=')?;
        (part_key == key).then(|| percent_decode(value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_detection_rejects_untrusted_hosts() {
        assert_eq!(
            provider_for_url("https://www.youtube.com/watch?v=abc"),
            Some(Provider::YouTube)
        );
        assert_eq!(
            provider_for_url("https://www.tiktok.com/@demo/video/123"),
            Some(Provider::TikTok)
        );
        assert_eq!(
            provider_for_url("https://www.facebook.com/reel/123"),
            Some(Provider::Facebook)
        );
        assert_eq!(provider_for_url("https://example.com/video"), None);
        assert_eq!(provider_for_url("file:///etc/passwd"), None);
    }

    #[test]
    fn extracts_balanced_json_object() {
        let html = r#"<script>var data = {"title":"a } brace","nested":{"ok":true}};</script>"#;
        let start = html.find("data").unwrap();

        assert_eq!(
            extract_json_object_after(html, start),
            Some(r#"{"title":"a } brace","nested":{"ok":true}}"#)
        );
    }
}
