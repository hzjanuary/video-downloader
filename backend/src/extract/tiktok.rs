use regex::Regex;
use serde_json::Value;

use crate::model::{Platform, StreamInfo, VideoInfo};

use super::{collect_u64, extract_json_object_after, first_string, ExtractError};

pub fn parse_tiktok(source_url: &str, html: &str) -> Result<VideoInfo, ExtractError> {
    let root = parse_state_json(html)?;
    let item = find_tiktok_item(&root).ok_or(ExtractError::MissingField("tiktok item"))?;
    let streams = parse_streams(item);

    if streams.is_empty() {
        return Err(ExtractError::NoStreams);
    }

    Ok(VideoInfo {
        platform: Platform::TikTok,
        source_url: source_url.to_string(),
        id: item
            .get("id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        title: item
            .get("desc")
            .or_else(|| item.get("description"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        author: parse_author(item),
        duration_seconds: item
            .get("video")
            .and_then(|video| collect_u64(video, &["duration"])),
        thumbnail_url: item
            .get("video")
            .and_then(|video| {
                video
                    .get("cover")
                    .or_else(|| video.get("originCover"))
                    .or_else(|| video.get("dynamicCover"))
            })
            .and_then(first_string),
        streams,
    })
}

fn parse_state_json(html: &str) -> Result<Value, ExtractError> {
    if let Some(json) = script_json_by_id(html, "SIGI_STATE") {
        return serde_json::from_str(json)
            .map_err(|error| ExtractError::InvalidJson(error.to_string()));
    }

    if let Some(json) = script_json_by_id(html, "__NEXT_DATA__") {
        return serde_json::from_str(json)
            .map_err(|error| ExtractError::InvalidJson(error.to_string()));
    }

    if let Some(json) = script_json_by_id(html, "__UNIVERSAL_DATA_FOR_REHYDRATION__") {
        return serde_json::from_str(json)
            .map_err(|error| ExtractError::InvalidJson(error.to_string()));
    }

    let marker = Regex::new(r#"SIGI_STATE"#).unwrap();
    if let Some(match_) = marker.find(html) {
        if let Some(json) = extract_json_object_after(html, match_.end()) {
            return serde_json::from_str(json)
                .map_err(|error| ExtractError::InvalidJson(error.to_string()));
        }
    }

    Err(ExtractError::MissingJson(
        "SIGI_STATE, __NEXT_DATA__, or __UNIVERSAL_DATA_FOR_REHYDRATION__",
    ))
}

fn script_json_by_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let pattern = Regex::new(&format!(
        r#"(?s)<script[^>]+id=["']{}["'][^>]*>(.*?)</script>"#,
        regex::escape(id)
    ))
    .unwrap();

    pattern
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|match_| match_.as_str().trim())
}

fn find_tiktok_item(value: &Value) -> Option<&Value> {
    if let Some(item_module) = value
        .get("ItemModule")
        .and_then(|module| module.as_object())
    {
        if let Some(item) = item_module
            .values()
            .find(|item| item.get("video").is_some())
        {
            return Some(item);
        }
    }

    if let Some(item_struct) = value.pointer("/props/pageProps/itemInfo/itemStruct") {
        return Some(item_struct);
    }

    find_object_with_video(value)
}

fn find_object_with_video(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(object) => {
            if object.get("video").is_some()
                && (object.get("id").is_some()
                    || object.get("desc").is_some()
                    || object.get("description").is_some())
            {
                return Some(value);
            }

            object.values().find_map(find_object_with_video)
        }
        Value::Array(items) => items.iter().find_map(find_object_with_video),
        _ => None,
    }
}

fn parse_author(item: &Value) -> Option<String> {
    item.get("author")
        .and_then(first_string)
        .or_else(|| item.get("authorId").and_then(first_string))
        .or_else(|| item.pointer("/authorStats/uniqueId").and_then(first_string))
        .or_else(|| item.pointer("/author/uniqueId").and_then(first_string))
}

fn parse_streams(item: &Value) -> Vec<StreamInfo> {
    let mut streams = Vec::new();
    let Some(video) = item.get("video") else {
        return streams;
    };

    push_stream(
        &mut streams,
        "play",
        video.get("playAddr").and_then(first_string),
        false,
    );

    if let Some(bit_rates) = video.get("bitRate").and_then(|value| value.as_array()) {
        for bit_rate in bit_rates {
            let url = bit_rate
                .pointer("/PlayAddr/UrlList")
                .and_then(first_string)
                .or_else(|| bit_rate.pointer("/playAddr/urlList").and_then(first_string));

            push_stream(
                &mut streams,
                bit_rate
                    .get("GearName")
                    .and_then(|value| value.as_str())
                    .unwrap_or("bitrate"),
                url,
                false,
            );
        }
    }

    push_stream(
        &mut streams,
        "download",
        video.get("downloadAddr").and_then(first_string),
        true,
    );

    streams
}

fn push_stream(streams: &mut Vec<StreamInfo>, quality: &str, url: Option<String>, watermark: bool) {
    let Some(url) = url else {
        return;
    };

    if streams.iter().any(|stream| stream.url == url) {
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
        watermark,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGI_HTML: &str = r#"
      <html>
        <script id="SIGI_STATE" type="application/json">
          {
            "ItemModule": {
              "7222222222222222222": {
                "id": "7222222222222222222",
                "desc": "Fixture TikTok Video",
                "author": "fixture_creator",
                "video": {
                  "duration": 12,
                  "cover": "https://p.example/cover.jpg",
                  "playAddr": "https://v.example/no-watermark.mp4",
                  "downloadAddr": "https://v.example/watermark.mp4",
                  "bitRate": [
                    {
                      "GearName": "720p",
                      "PlayAddr": {
                        "UrlList": ["https://v.example/no-watermark-720.mp4"]
                      }
                    }
                  ]
                }
              }
            }
          }
        </script>
      </html>
    "#;

    const NEXT_DATA_HTML: &str = r#"
      <html>
        <script id="__NEXT_DATA__" type="application/json">
          {
            "props": {
              "pageProps": {
                "itemInfo": {
                  "itemStruct": {
                    "id": "7333333333333333333",
                    "desc": "Next Data TikTok",
                    "author": {"uniqueId": "next_creator"},
                    "video": {
                      "duration": 9,
                      "cover": "https://p.example/next-cover.jpg",
                      "playAddr": ["https://v.example/next-no-watermark.mp4"]
                    }
                  }
                }
              }
            }
          }
        </script>
      </html>
    "#;

    #[test]
    fn parses_sigi_state() {
        let video = parse_tiktok(
            "https://www.tiktok.com/@fixture/video/7222222222222222222",
            SIGI_HTML,
        )
        .unwrap();

        assert_eq!(video.platform, Platform::TikTok);
        assert_eq!(video.id.as_deref(), Some("7222222222222222222"));
        assert_eq!(video.title.as_deref(), Some("Fixture TikTok Video"));
        assert_eq!(video.author.as_deref(), Some("fixture_creator"));
        assert_eq!(video.duration_seconds, Some(12));
        assert_eq!(
            video.thumbnail_url.as_deref(),
            Some("https://p.example/cover.jpg")
        );
        assert_eq!(video.streams[0].url, "https://v.example/no-watermark.mp4");
        assert!(!video.streams[0].watermark);
        assert_eq!(video.streams[1].quality.as_deref(), Some("720p"));
    }

    #[test]
    fn parses_next_data() {
        let video = parse_tiktok(
            "https://www.tiktok.com/@fixture/video/7333333333333333333",
            NEXT_DATA_HTML,
        )
        .unwrap();

        assert_eq!(video.id.as_deref(), Some("7333333333333333333"));
        assert_eq!(video.title.as_deref(), Some("Next Data TikTok"));
        assert_eq!(video.author.as_deref(), Some("next_creator"));
        assert_eq!(
            video.streams[0].url,
            "https://v.example/next-no-watermark.mp4"
        );
    }
}
