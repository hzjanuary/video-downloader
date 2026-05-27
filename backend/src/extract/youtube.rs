use regex::Regex;
use serde_json::Value;

use crate::model::{Platform, StreamInfo, VideoInfo};

use super::{
    collect_string, collect_u64, extract_json_object_after, last_thumbnail_url, query_param,
    ExtractError,
};

pub fn parse_youtube(source_url: &str, html: &str) -> Result<VideoInfo, ExtractError> {
    let response = parse_player_response(html)?;
    let details = response
        .get("videoDetails")
        .ok_or(ExtractError::MissingField("videoDetails"))?;
    let streams = parse_streams(&response)?;

    if streams.is_empty() {
        return Err(ExtractError::NoStreams);
    }

    Ok(VideoInfo {
        platform: Platform::YouTube,
        source_url: source_url.to_string(),
        id: collect_string(details, &["videoId"]),
        title: collect_string(details, &["title"]),
        author: collect_string(details, &["author"]),
        duration_seconds: collect_u64(details, &["lengthSeconds"]),
        thumbnail_url: last_thumbnail_url(details),
        streams,
    })
}

fn parse_player_response(html: &str) -> Result<Value, ExtractError> {
    let marker = Regex::new(r#"ytInitialPlayerResponse\s*="#).unwrap();
    let start = marker
        .find(html)
        .map(|match_| match_.end())
        .ok_or(ExtractError::MissingJson("ytInitialPlayerResponse"))?;
    let json = extract_json_object_after(html, start)
        .ok_or(ExtractError::MissingJson("ytInitialPlayerResponse"))?;

    serde_json::from_str(json).map_err(|error| ExtractError::InvalidJson(error.to_string()))
}

fn parse_streams(response: &Value) -> Result<Vec<StreamInfo>, ExtractError> {
    let streaming_data = response
        .get("streamingData")
        .ok_or(ExtractError::MissingField("streamingData"))?;
    let mut streams = Vec::new();

    for section in ["formats", "adaptiveFormats"] {
        if let Some(items) = streaming_data
            .get(section)
            .and_then(|value| value.as_array())
        {
            for item in items {
                if let Some(stream) = parse_stream(item) {
                    streams.push(stream);
                }
            }
        }
    }

    Ok(streams)
}

fn parse_stream(item: &Value) -> Option<StreamInfo> {
    let url = item
        .get("signatureCipher")
        .or_else(|| item.get("cipher"))
        .and_then(|value| value.as_str())
        .and_then(signed_cipher_url)
        .or_else(|| {
            item.get("url")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })?;
    let mime_type = item
        .get("mimeType")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let width = item
        .get("width")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok());
    let height = item
        .get("height")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok());
    let has_video = mime_type
        .as_deref()
        .is_none_or(|mime| mime.starts_with("video/"));
    let has_audio = item.get("audioQuality").is_some()
        || mime_type
            .as_deref()
            .is_some_and(|mime| mime.starts_with("audio/"));

    Some(StreamInfo {
        url,
        mime_type,
        quality: item
            .get("qualityLabel")
            .or_else(|| item.get("quality"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        width,
        height,
        bitrate: item.get("bitrate").and_then(|value| value.as_u64()),
        has_audio,
        has_video,
        watermark: false,
    })
}

fn signed_cipher_url(cipher: &str) -> Option<String> {
    let mut url = query_param(cipher, "url")?;
    let signature = query_param(cipher, "s")?;
    let signature_param = query_param(cipher, "sp").unwrap_or_else(|| "signature".to_string());
    let signature = decipher_signature(&signature);
    let separator = if url.contains('?') { '&' } else { '?' };

    url.push(separator);
    url.push_str("alr=yes&");
    url.push_str(&signature_param);
    url.push('=');
    url.push_str(&percent_encode_component(&signature));

    Some(url)
}

fn decipher_signature(signature: &str) -> String {
    if signature.len() < 20 {
        return signature.to_string();
    }

    let mut chars: Vec<char> = signature.chars().collect();

    chars.pop();
    swap(&mut chars, 18);
    swap(&mut chars, 68);
    swap(&mut chars, 20);
    swap(&mut chars, 82);
    chars.pop();
    swap(&mut chars, 64);
    chars.pop();

    if !chars.is_empty() {
        chars.remove(0);
    }

    chars.into_iter().collect()
}

fn swap(chars: &mut [char], index: usize) {
    if chars.is_empty() {
        return;
    }

    let target = index % chars.len();
    chars.swap(0, target);
}

fn percent_encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    const YOUTUBE_HTML: &str = r#"
        <html>
          <script>
            var ytInitialPlayerResponse = {
              "videoDetails": {
                "videoId": "abc123",
                "title": "Fixture YouTube Video",
                "author": "Fixture Channel",
                "lengthSeconds": "61",
                "thumbnail": {
                  "thumbnails": [
                    {"url": "https://img.example/small.jpg"},
                    {"url": "https://img.example/large.jpg"}
                  ]
                }
              },
              "streamingData": {
                "formats": [
                  {
                    "url": "https://video.example/itag18.mp4",
                    "mimeType": "video/mp4",
                    "qualityLabel": "360p",
                    "width": 640,
                    "height": 360,
                    "bitrate": 400000,
                    "audioQuality": "AUDIO_QUALITY_MEDIUM"
                  }
                ],
                "adaptiveFormats": [
                  {
                    "signatureCipher": "url=https%3A%2F%2Fvideo.example%2Fitag137.mp4%3Fitag%3D137&sp=sig&s=abc",
                    "mimeType": "video/mp4",
                    "qualityLabel": "1080p",
                    "width": 1920,
                    "height": 1080,
                    "bitrate": 2500000
                  }
                ]
              }
            };
          </script>
        </html>
    "#;

    #[test]
    fn parses_youtube_player_response() {
        let video = parse_youtube("https://www.youtube.com/watch?v=abc123", YOUTUBE_HTML).unwrap();

        assert_eq!(video.platform, Platform::YouTube);
        assert_eq!(video.id.as_deref(), Some("abc123"));
        assert_eq!(video.title.as_deref(), Some("Fixture YouTube Video"));
        assert_eq!(video.author.as_deref(), Some("Fixture Channel"));
        assert_eq!(video.duration_seconds, Some(61));
        assert_eq!(
            video.thumbnail_url.as_deref(),
            Some("https://img.example/large.jpg")
        );
        assert_eq!(video.streams.len(), 2);
        assert_eq!(video.streams[0].quality.as_deref(), Some("360p"));
        assert_eq!(video.streams[1].quality.as_deref(), Some("1080p"));
        assert_eq!(
            video.streams[1].url,
            "https://video.example/itag137.mp4?itag=137&alr=yes&sig=abc"
        );
    }
}
