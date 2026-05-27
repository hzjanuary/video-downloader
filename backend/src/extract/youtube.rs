use reqwest::Client;
use serde_json::{json, Value};

use crate::model::{Platform, StreamInfo, VideoInfo};

use super::{collect_string, collect_u64, last_thumbnail_url, query_param, ExtractError};

const INNERTUBE_PLAYER_ENDPOINT: &str = "https://www.youtube.com/youtubei/v1/player";
const ANDROID_CLIENT_NAME: &str = "ANDROID";
const ANDROID_CLIENT_VERSION: &str = "20.10.38";
const ANDROID_CLIENT_ID: &str = "3";
const ANDROID_SDK_VERSION: u32 = 35;
const ANDROID_USER_AGENT: &str = "com.google.android.youtube/20.10.38 (Linux; U; Android 14)";

pub async fn fetch_youtube(
    client: &Client,
    source_url: &str,
    cookie: Option<&str>,
) -> Result<VideoInfo, ExtractError> {
    let video_id = video_id_from_url(source_url).ok_or(ExtractError::MissingField("videoId"))?;
    let payload = player_payload(&video_id);
    let mut request = client
        .post(INNERTUBE_PLAYER_ENDPOINT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::USER_AGENT, ANDROID_USER_AGENT)
        .header("x-youtube-client-name", ANDROID_CLIENT_ID)
        .header("x-youtube-client-version", ANDROID_CLIENT_VERSION)
        .json(&payload);

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
            "YouTube InnerTube player returned status {status}"
        )));
    }

    let body = response
        .json::<Value>()
        .await
        .map_err(|error| ExtractError::InvalidJson(error.to_string()))?;

    parse_youtube_player_response(source_url, &body)
}

pub fn parse_youtube_player_response(
    source_url: &str,
    response: &Value,
) -> Result<VideoInfo, ExtractError> {
    reject_unplayable(response)?;

    let details = response
        .get("videoDetails")
        .ok_or(ExtractError::MissingField("videoDetails"))?;
    let streams = parse_streams(response)?;

    if streams.is_empty() {
        return Err(ExtractError::NoStreams);
    }

    Ok(VideoInfo {
        platform: Platform::YouTube,
        source_url: source_url.to_string(),
        id: collect_string(details, &["videoId"]).or_else(|| video_id_from_url(source_url)),
        title: collect_string(details, &["title"]),
        author: collect_string(details, &["author"]),
        duration_seconds: collect_u64(details, &["lengthSeconds"]),
        thumbnail_url: last_thumbnail_url(details),
        streams,
    })
}

pub fn video_id_from_url(source_url: &str) -> Option<String> {
    let without_scheme = source_url
        .strip_prefix("https://")
        .or_else(|| source_url.strip_prefix("http://"))?;
    let (authority, remainder) = without_scheme
        .split_once('/')
        .map_or((without_scheme, ""), |(authority, path)| (authority, path));
    let host = authority
        .rsplit('@')
        .next()?
        .split(':')
        .next()?
        .to_ascii_lowercase();
    let path = remainder.split(['?', '#']).next().unwrap_or("");
    let query = source_url
        .split_once('?')
        .map(|(_, query)| query.split('#').next().unwrap_or(query));

    if host == "youtu.be" {
        return path
            .split('/')
            .next()
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned);
    }

    if host == "youtube.com" || host.ends_with(".youtube.com") {
        if let Some(video_id) = query.and_then(|query| query_param(query, "v")) {
            if !video_id.is_empty() {
                return Some(video_id);
            }
        }

        for prefix in ["shorts/", "embed/", "live/"] {
            if let Some(id) = path.strip_prefix(prefix) {
                return id
                    .split('/')
                    .next()
                    .filter(|id| !id.is_empty())
                    .map(ToOwned::to_owned);
            }
        }
    }

    None
}

fn player_payload(video_id: &str) -> Value {
    json!({
        "videoId": video_id,
        "context": {
            "client": {
                "clientName": ANDROID_CLIENT_NAME,
                "clientVersion": ANDROID_CLIENT_VERSION,
                "androidSdkVersion": ANDROID_SDK_VERSION,
                "hl": "en",
                "gl": "US"
            }
        },
        "contentCheckOk": true,
        "racyCheckOk": true
    })
}

fn reject_unplayable(response: &Value) -> Result<(), ExtractError> {
    let Some(playability) = response.get("playabilityStatus") else {
        return Ok(());
    };
    let status = playability
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("UNKNOWN");

    if status == "OK" {
        return Ok(());
    }

    let reason = playability
        .get("reason")
        .and_then(|value| value.as_str())
        .unwrap_or("no reason provided");

    Err(ExtractError::FetchFailed(format!(
        "YouTube video is not playable through InnerTube ({status}): {reason}"
    )))
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
    let url = item.get("url").and_then(|value| value.as_str())?;
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
        .is_some_and(|mime| mime.starts_with("video/"));
    let has_audio = item.get("audioQuality").is_some()
        || mime_type
            .as_deref()
            .is_some_and(|mime| mime.starts_with("audio/"));

    Some(StreamInfo {
        url: url.to_string(),
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

fn clean_cookie(cookie: Option<&str>) -> Option<&str> {
    cookie.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const YOUTUBE_PLAYER_RESPONSE: &str = r#"
        {
          "playabilityStatus": {
            "status": "OK"
          },
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
                "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
                "qualityLabel": "360p",
                "width": 640,
                "height": 360,
                "bitrate": 400000,
                "audioQuality": "AUDIO_QUALITY_MEDIUM"
              }
            ],
            "adaptiveFormats": [
              {
                "url": "https://video.example/itag137.mp4",
                "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                "qualityLabel": "1080p",
                "width": 1920,
                "height": 1080,
                "bitrate": 2500000
              }
            ]
          }
        }
    "#;

    #[test]
    fn parses_youtube_player_response() {
        let response: Value = serde_json::from_str(YOUTUBE_PLAYER_RESPONSE).unwrap();
        let video =
            parse_youtube_player_response("https://www.youtube.com/watch?v=abc123", &response)
                .unwrap();

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
        assert_eq!(video.streams[0].has_audio, true);
        assert_eq!(video.streams[0].has_video, true);
        assert_eq!(video.streams[1].quality.as_deref(), Some("1080p"));
        assert_eq!(video.streams[1].url, "https://video.example/itag137.mp4");
    }

    #[test]
    fn extracts_video_id_from_supported_urls() {
        assert_eq!(
            video_id_from_url("https://www.youtube.com/watch?v=abc123&t=1"),
            Some("abc123".to_string())
        );
        assert_eq!(
            video_id_from_url("https://youtu.be/abc123?si=demo"),
            Some("abc123".to_string())
        );
        assert_eq!(
            video_id_from_url("https://www.youtube.com/shorts/abc123"),
            Some("abc123".to_string())
        );
    }
}
