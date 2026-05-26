use regex::Regex;
use serde_json::Value;

use crate::model::{Platform, StreamInfo, VideoInfo};

use super::{first_string, ExtractError};

const STREAM_KEYS: &[&str] = &[
    "browser_native_hd_url",
    "browser_native_sd_url",
    "playable_url_quality_hd",
    "playable_url",
    "hd_src",
    "sd_src",
    "contentUrl",
];

pub fn parse_facebook(source_url: &str, html: &str) -> Result<VideoInfo, ExtractError> {
    let decoded_html = decode_html_entities(html);
    let mut streams = Vec::new();

    collect_meta_streams(html, &mut streams);
    collect_meta_streams(&decoded_html, &mut streams);
    collect_raw_stream_fields(html, &mut streams);
    collect_raw_stream_fields(&decoded_html, &mut streams);

    let json_values = parse_json_scripts(html);
    for value in &json_values {
        collect_streams_from_json(value, &mut streams);
    }

    if streams.is_empty() {
        return Err(ExtractError::NoStreams);
    }

    let title = meta_content(&decoded_html, &["og:title", "twitter:title"])
        .or_else(|| title_tag(&decoded_html))
        .or_else(|| json_values.iter().find_map(json_title));
    let thumbnail_url = meta_content(&decoded_html, &["og:image", "twitter:image"])
        .or_else(|| json_values.iter().find_map(json_thumbnail));
    let duration_seconds = json_values
        .iter()
        .find_map(json_duration)
        .or_else(|| raw_duration_seconds(&decoded_html));

    Ok(VideoInfo {
        platform: Platform::Facebook,
        source_url: source_url.to_string(),
        id: facebook_id_from_url(source_url).or_else(|| facebook_id_from_html(&decoded_html)),
        title,
        author: json_values.iter().find_map(json_author),
        duration_seconds,
        thumbnail_url,
        streams,
    })
}

fn collect_meta_streams(html: &str, streams: &mut Vec<StreamInfo>) {
    for property in ["og:video", "og:video:url", "og:video:secure_url"] {
        if let Some(url) = meta_content(html, &[property]) {
            push_stream(streams, property, Some(url));
        }
    }
}

fn collect_raw_stream_fields(html: &str, streams: &mut Vec<StreamInfo>) {
    for key in STREAM_KEYS {
        let quoted = Regex::new(&format!(
            r#""{}"\s*:\s*"((?:\\.|[^"\\])*)""#,
            regex::escape(key)
        ))
        .unwrap();

        for captures in quoted.captures_iter(html) {
            let raw = captures.get(1).map(|match_| match_.as_str());
            push_stream(streams, key, raw.and_then(decode_json_string));
        }

        let entity_quoted = Regex::new(&format!(
            r#"&quot;{}&quot;\s*:\s*&quot;(.*?)&quot;"#,
            regex::escape(key)
        ))
        .unwrap();

        for captures in entity_quoted.captures_iter(html) {
            let raw = captures.get(1).map(|match_| match_.as_str());
            push_stream(streams, key, raw.map(decode_html_entities));
        }
    }
}

fn collect_streams_from_json(value: &Value, streams: &mut Vec<StreamInfo>) {
    match value {
        Value::Object(object) => {
            for key in STREAM_KEYS {
                if let Some(url) = object.get(*key).and_then(first_string) {
                    push_stream(streams, key, Some(url));
                }
            }

            for item in object.values() {
                collect_streams_from_json(item, streams);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_streams_from_json(item, streams);
            }
        }
        _ => {}
    }
}

fn push_stream(streams: &mut Vec<StreamInfo>, quality: &str, url: Option<String>) {
    let Some(url) = url.map(|value| decode_html_entities(&value)) else {
        return;
    };
    let url = url.trim().to_string();

    if !looks_like_video_url(&url) || streams.iter().any(|stream| stream.url == url) {
        return;
    }

    streams.push(StreamInfo {
        url,
        mime_type: Some("video/mp4".to_string()),
        quality: Some(quality.to_string()),
        width: None,
        height: None,
        bitrate: None,
        has_audio: true,
        has_video: true,
        watermark: false,
    });
}

fn looks_like_video_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();

    lower.starts_with("http")
        && (lower.contains(".mp4")
            || lower.contains("video")
            || lower.contains("fbcdn")
            || lower.contains("contenturl"))
}

fn parse_json_scripts(html: &str) -> Vec<Value> {
    let script = Regex::new(r#"(?s)<script[^>]*>(.*?)</script>"#).unwrap();
    let mut values = Vec::new();

    for captures in script.captures_iter(html) {
        let Some(match_) = captures.get(1) else {
            continue;
        };
        let mut text = match_.as_str().trim();

        if let Some(stripped) = text.strip_prefix("for (;;);") {
            text = stripped.trim();
        }

        if !text.starts_with('{') && !text.starts_with('[') {
            continue;
        }

        if let Ok(value) = serde_json::from_str(text) {
            values.push(value);
        }
    }

    values
}

fn meta_content(html: &str, properties: &[&str]) -> Option<String> {
    for property in properties {
        let property_pattern = regex::escape(property);
        let patterns = [
            format!(
                r#"(?is)<meta[^>]+(?:property|name)=["']{}["'][^>]+content=["']([^"']+)["'][^>]*>"#,
                property_pattern
            ),
            format!(
                r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+(?:property|name)=["']{}["'][^>]*>"#,
                property_pattern
            ),
        ];

        for pattern in patterns {
            let regex = Regex::new(&pattern).unwrap();
            if let Some(content) = regex
                .captures(html)
                .and_then(|captures| captures.get(1))
                .map(|match_| decode_html_entities(match_.as_str()))
            {
                return Some(content);
            }
        }
    }

    None
}

fn title_tag(html: &str) -> Option<String> {
    Regex::new(r#"(?is)<title[^>]*>(.*?)</title>"#)
        .unwrap()
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|match_| decode_html_entities(match_.as_str()).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn json_title(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in ["name", "headline", "title", "description"] {
                if let Some(found) = object.get(key).and_then(first_string) {
                    return Some(found);
                }
            }

            object.values().find_map(json_title)
        }
        Value::Array(items) => items.iter().find_map(json_title),
        _ => None,
    }
}

fn json_author(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(found) = object
                .get("author")
                .and_then(|author| author.get("name"))
                .and_then(first_string)
            {
                return Some(found);
            }

            if let Some(found) = object.get("owner").and_then(|owner| {
                owner
                    .get("name")
                    .or_else(|| owner.get("title"))
                    .and_then(first_string)
            }) {
                return Some(found);
            }

            object.values().find_map(json_author)
        }
        Value::Array(items) => items.iter().find_map(json_author),
        _ => None,
    }
}

fn json_thumbnail(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(found) = object
                .get("thumbnailUrl")
                .or_else(|| object.get("thumbnail_url"))
                .or_else(|| object.get("image"))
                .and_then(first_string)
            {
                return Some(found);
            }

            object.values().find_map(json_thumbnail)
        }
        Value::Array(items) => items.iter().find_map(json_thumbnail),
        _ => None,
    }
}

fn json_duration(value: &Value) -> Option<u64> {
    match value {
        Value::Object(object) => {
            for key in ["duration", "duration_seconds", "playable_duration_in_ms"] {
                if let Some(found) = object.get(key).and_then(value_as_duration_seconds) {
                    return Some(found);
                }
            }

            object.values().find_map(json_duration)
        }
        Value::Array(items) => items.iter().find_map(json_duration),
        _ => None,
    }
}

fn value_as_duration_seconds(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| parse_iso8601_duration_seconds(raw))
        })
        .map(|seconds| {
            if seconds > 10_000 {
                seconds / 1000
            } else {
                seconds
            }
        })
}

fn raw_duration_seconds(html: &str) -> Option<u64> {
    for key in ["duration", "duration_seconds", "playable_duration_in_ms"] {
        let regex = Regex::new(&format!(r#""{}"\s*:\s*"?([0-9]+)"?"#, regex::escape(key))).unwrap();

        if let Some(seconds) = regex
            .captures(html)
            .and_then(|captures| captures.get(1))
            .and_then(|match_| match_.as_str().parse::<u64>().ok())
            .map(|value| if value > 10_000 { value / 1000 } else { value })
        {
            return Some(seconds);
        }
    }

    None
}

fn parse_iso8601_duration_seconds(raw: &str) -> Option<u64> {
    let captures = Regex::new(r#"^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$"#)
        .unwrap()
        .captures(raw)?;
    let hours = captures
        .get(1)
        .and_then(|match_| match_.as_str().parse::<u64>().ok())
        .unwrap_or(0);
    let minutes = captures
        .get(2)
        .and_then(|match_| match_.as_str().parse::<u64>().ok())
        .unwrap_or(0);
    let seconds = captures
        .get(3)
        .and_then(|match_| match_.as_str().parse::<u64>().ok())
        .unwrap_or(0);

    Some(hours * 3600 + minutes * 60 + seconds)
}

fn facebook_id_from_url(source_url: &str) -> Option<String> {
    for pattern in [
        r#"/(?:reel|videos)/(\d+)"#,
        r#"[?&]v=(\d+)"#,
        r#"[?&]video_id=(\d+)"#,
    ] {
        let regex = Regex::new(pattern).unwrap();
        if let Some(id) = regex
            .captures(source_url)
            .and_then(|captures| captures.get(1))
            .map(|match_| match_.as_str().to_string())
        {
            return Some(id);
        }
    }

    None
}

fn facebook_id_from_html(html: &str) -> Option<String> {
    for key in ["videoID", "video_id", "videoId"] {
        let regex =
            Regex::new(&format!(r#""{}"\s*:\s*"?(\d{{5,}})"?"#, regex::escape(key))).unwrap();

        if let Some(id) = regex
            .captures(html)
            .and_then(|captures| captures.get(1))
            .map(|match_| match_.as_str().to_string())
        {
            return Some(id);
        }
    }

    None
}

fn decode_json_string(raw: &str) -> Option<String> {
    let quoted = format!("\"{raw}\"");

    serde_json::from_str::<String>(&quoted)
        .ok()
        .map(|value| decode_html_entities(&value))
}

fn decode_html_entities(input: &str) -> String {
    let mut output = input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#034;", "\"")
        .replace("&#34;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    let numeric = Regex::new(r#"&#x([0-9a-fA-F]+);|&#([0-9]+);"#).unwrap();
    loop {
        let next = numeric
            .replace_all(&output, |captures: &regex::Captures| {
                let value = captures
                    .get(1)
                    .and_then(|match_| u32::from_str_radix(match_.as_str(), 16).ok())
                    .or_else(|| {
                        captures
                            .get(2)
                            .and_then(|match_| match_.as_str().parse::<u32>().ok())
                    })
                    .and_then(char::from_u32);

                value
                    .map(|character| character.to_string())
                    .unwrap_or_else(|| captures.get(0).unwrap().as_str().to_string())
            })
            .into_owned();

        if next == output {
            return output;
        }

        output = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACEBOOK_HTML: &str = r#"
      <html>
        <head>
          <meta property="og:title" content="Fixture Facebook Reel" />
          <meta property="og:image" content="https://img.example/fb.jpg" />
        </head>
        <body>
          <script>
            require("RelayPrefetchedStreamCache").next({
              "__bbox": {
                "result": {
                  "data": {
                    "video": {
                      "id": "123456789012345",
                      "owner": {"name": "Fixture Page"},
                      "playable_duration_in_ms": 42000,
                      "browser_native_hd_url": "https:\/\/video.xx.fbcdn.net\/v\/fixture-hd.mp4?token=abc\u0026bytestart=0",
                      "browser_native_sd_url": "https:\/\/video.xx.fbcdn.net\/v\/fixture-sd.mp4?token=abc"
                    }
                  }
                }
              }
            });
          </script>
        </body>
      </html>
    "#;

    const JSON_LD_HTML: &str = r#"
      <html>
        <script type="application/ld+json">
          {
            "@type": "VideoObject",
            "name": "JSON-LD Facebook Video",
            "duration": "PT1M05S",
            "thumbnailUrl": "https://img.example/jsonld.jpg",
            "contentUrl": "https://video.xx.fbcdn.net/v/jsonld.mp4"
          }
        </script>
      </html>
    "#;

    #[test]
    fn parses_facebook_native_mp4_urls() {
        let video = parse_facebook(
            "https://www.facebook.com/reel/123456789012345",
            FACEBOOK_HTML,
        )
        .unwrap();

        assert_eq!(video.platform, Platform::Facebook);
        assert_eq!(video.id.as_deref(), Some("123456789012345"));
        assert_eq!(video.title.as_deref(), Some("Fixture Facebook Reel"));
        assert_eq!(video.duration_seconds, Some(42));
        assert_eq!(
            video.thumbnail_url.as_deref(),
            Some("https://img.example/fb.jpg")
        );
        assert_eq!(video.streams.len(), 2);
        assert_eq!(
            video.streams[0].url,
            "https://video.xx.fbcdn.net/v/fixture-hd.mp4?token=abc&bytestart=0"
        );
    }

    #[test]
    fn parses_facebook_json_ld_content_url() {
        let video = parse_facebook(
            "https://www.facebook.com/watch/?v=987654321098765",
            JSON_LD_HTML,
        )
        .unwrap();

        assert_eq!(video.id.as_deref(), Some("987654321098765"));
        assert_eq!(video.title.as_deref(), Some("JSON-LD Facebook Video"));
        assert_eq!(video.duration_seconds, Some(65));
        assert_eq!(
            video.streams[0].url,
            "https://video.xx.fbcdn.net/v/jsonld.mp4"
        );
    }
}
