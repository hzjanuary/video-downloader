use std::{collections::HashSet, time::Duration};

#[cfg(test)]
use std::{collections::HashMap, sync::Arc};

use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::model::ChannelVideo;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/125.0 Safari/537.36 VideoDownloader/0.1";
const MAX_PAGES: usize = 50;
const RETRIES: usize = 3;

#[derive(Debug)]
pub enum ChannelError {
    UnsupportedUrl,
    FetchFailed(String),
    MissingJson(&'static str),
    InvalidJson(String),
    MissingField(&'static str),
    NoVideos,
}

impl ChannelError {
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedUrl => {
                "unsupported channel url; expected YouTube channel/playlist or TikTok profile"
                    .to_string()
            }
            Self::FetchFailed(error) => format!("failed to fetch channel page: {error}"),
            Self::MissingJson(name) => format!("could not find {name} json in channel page"),
            Self::InvalidJson(error) => format!("invalid provider channel json: {error}"),
            Self::MissingField(field) => format!("missing provider channel field: {field}"),
            Self::NoVideos => "no videos found".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    YouTube,
    TikTok,
}

#[derive(Clone)]
pub struct ChannelFetcher {
    fetcher: PageFetcher,
}

#[derive(Clone)]
enum PageFetcher {
    Live(Client),
    #[cfg(test)]
    Fixture(Arc<HashMap<String, String>>),
}

#[derive(Debug, Clone)]
struct YouTubeContinuation {
    token: String,
    api_key: Option<String>,
    context: Value,
}

#[derive(Debug, Clone)]
struct TikTokCursor {
    sec_uid: Option<String>,
    cursor: String,
}

#[derive(Debug, Clone)]
struct TikTokPage {
    videos: Vec<ChannelVideo>,
    cursor: Option<String>,
    has_more: bool,
    sec_uid: Option<String>,
}

impl ChannelFetcher {
    pub fn live() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(25))
            .user_agent(USER_AGENT)
            .build()?;

        Ok(Self {
            fetcher: PageFetcher::Live(client),
        })
    }

    #[cfg(test)]
    pub fn fixture(fixtures: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            fetcher: PageFetcher::Fixture(Arc::new(fixtures.into_iter().collect())),
        }
    }

    pub async fn fetch_channel(&self, source_url: &str) -> Result<Vec<ChannelVideo>, ChannelError> {
        match provider_for_url(source_url).ok_or(ChannelError::UnsupportedUrl)? {
            Provider::YouTube => self.fetch_youtube_channel(source_url).await,
            Provider::TikTok => self.fetch_tiktok_profile(source_url).await,
        }
    }

    async fn fetch_youtube_channel(
        &self,
        source_url: &str,
    ) -> Result<Vec<ChannelVideo>, ChannelError> {
        let html = self.fetch_get(source_url).await?;
        let initial = parse_youtube_initial(&html)?;
        let api_key = parse_youtube_api_key(&html);
        let context = parse_youtube_context(&html).unwrap_or_else(default_youtube_context);

        let mut videos = Vec::new();
        let mut seen_videos = HashSet::new();
        let mut seen_tokens = HashSet::new();
        let mut continuation = collect_youtube_page(&initial, &mut videos, &mut seen_videos)
            .into_iter()
            .find(|token| seen_tokens.insert(token.clone()))
            .map(|token| YouTubeContinuation {
                token,
                api_key,
                context,
            });

        let mut pages = 1usize;

        while let Some(request) = continuation.take() {
            if pages >= MAX_PAGES {
                break;
            }

            self.delay_between_pages().await;
            let page = self.fetch_youtube_continuation(&request).await?;
            let value: Value = serde_json::from_str(&page)
                .map_err(|error| ChannelError::InvalidJson(error.to_string()))?;
            let next_tokens = collect_youtube_page(&value, &mut videos, &mut seen_videos);
            continuation = next_tokens
                .into_iter()
                .find(|token| seen_tokens.insert(token.clone()))
                .map(|token| YouTubeContinuation {
                    token,
                    api_key: request.api_key.clone(),
                    context: request.context.clone(),
                });
            pages += 1;
        }

        if videos.is_empty() {
            return Err(ChannelError::NoVideos);
        }

        Ok(videos)
    }

    async fn fetch_tiktok_profile(
        &self,
        source_url: &str,
    ) -> Result<Vec<ChannelVideo>, ChannelError> {
        let html = self.fetch_get(source_url).await?;
        let initial_json = parse_tiktok_state(&html)?;
        let initial_page = parse_tiktok_page(&initial_json);
        let mut videos = Vec::new();
        let mut seen_videos = HashSet::new();

        push_unique_videos(&mut videos, &mut seen_videos, initial_page.videos);

        let mut cursor = initial_page
            .has_more
            .then_some(initial_page.cursor)
            .flatten()
            .map(|cursor| TikTokCursor {
                sec_uid: initial_page.sec_uid,
                cursor,
            });
        let mut seen_cursors = HashSet::new();
        let mut pages = 1usize;

        while let Some(request) = cursor.take() {
            if pages >= MAX_PAGES || !seen_cursors.insert(request.cursor.clone()) {
                break;
            }

            self.delay_between_pages().await;
            let page = self.fetch_tiktok_cursor(&request).await?;
            let value: Value = serde_json::from_str(&page)
                .map_err(|error| ChannelError::InvalidJson(error.to_string()))?;
            let parsed = parse_tiktok_page(&value);
            push_unique_videos(&mut videos, &mut seen_videos, parsed.videos);

            cursor = parsed
                .has_more
                .then_some(parsed.cursor)
                .flatten()
                .map(|next| TikTokCursor {
                    sec_uid: parsed.sec_uid.or_else(|| request.sec_uid.clone()),
                    cursor: next,
                });
            pages += 1;
        }

        if videos.is_empty() {
            return Err(ChannelError::NoVideos);
        }

        Ok(videos)
    }

    async fn fetch_get(&self, url: &str) -> Result<String, ChannelError> {
        match &self.fetcher {
            PageFetcher::Live(client) => {
                let mut last_error = None;

                for attempt in 0..RETRIES {
                    if attempt > 0 {
                        sleep(retry_delay(attempt)).await;
                    }

                    match client.get(url).send().await {
                        Ok(response) if response.status().is_success() => {
                            return response
                                .text()
                                .await
                                .map_err(|error| ChannelError::FetchFailed(error.to_string()));
                        }
                        Ok(response) => {
                            last_error = Some(format!("provider returned {}", response.status()));
                        }
                        Err(error) => last_error = Some(error.to_string()),
                    }
                }

                Err(ChannelError::FetchFailed(
                    last_error.unwrap_or_else(|| "unknown provider error".to_string()),
                ))
            }
            #[cfg(test)]
            PageFetcher::Fixture(fixtures) => fixtures
                .get(url)
                .cloned()
                .ok_or_else(|| ChannelError::FetchFailed(format!("fixture not found: {url}"))),
        }
    }

    async fn fetch_youtube_continuation(
        &self,
        request: &YouTubeContinuation,
    ) -> Result<String, ChannelError> {
        match &self.fetcher {
            PageFetcher::Live(client) => {
                let api_key = request
                    .api_key
                    .as_deref()
                    .ok_or(ChannelError::MissingField("INNERTUBE_API_KEY"))?;
                let endpoint = format!("https://www.youtube.com/youtubei/v1/browse?key={api_key}");
                let payload = json!({
                    "context": request.context,
                    "continuation": request.token
                });
                let mut last_error = None;

                for attempt in 0..RETRIES {
                    if attempt > 0 {
                        sleep(retry_delay(attempt)).await;
                    }

                    match client.post(&endpoint).json(&payload).send().await {
                        Ok(response) if response.status().is_success() => {
                            return response
                                .text()
                                .await
                                .map_err(|error| ChannelError::FetchFailed(error.to_string()));
                        }
                        Ok(response) => {
                            last_error = Some(format!("provider returned {}", response.status()));
                        }
                        Err(error) => last_error = Some(error.to_string()),
                    }
                }

                Err(ChannelError::FetchFailed(last_error.unwrap_or_else(|| {
                    "unknown YouTube continuation error".to_string()
                })))
            }
            #[cfg(test)]
            PageFetcher::Fixture(fixtures) => fixtures
                .get(&format!("youtube:continuation:{}", request.token))
                .cloned()
                .ok_or_else(|| {
                    ChannelError::FetchFailed(format!(
                        "fixture not found: youtube:continuation:{}",
                        request.token
                    ))
                }),
        }
    }

    async fn fetch_tiktok_cursor(&self, request: &TikTokCursor) -> Result<String, ChannelError> {
        match &self.fetcher {
            PageFetcher::Live(client) => {
                let sec_uid = request
                    .sec_uid
                    .as_deref()
                    .ok_or(ChannelError::MissingField("secUid"))?;
                let endpoint = format!(
                    "https://www.tiktok.com/api/post/item_list/?aid=1988&count=35&cursor={}&secUid={}",
                    request.cursor, sec_uid
                );
                let mut last_error = None;

                for attempt in 0..RETRIES {
                    if attempt > 0 {
                        sleep(retry_delay(attempt)).await;
                    }

                    match client.get(&endpoint).send().await {
                        Ok(response) if response.status().is_success() => {
                            return response
                                .text()
                                .await
                                .map_err(|error| ChannelError::FetchFailed(error.to_string()));
                        }
                        Ok(response) => {
                            last_error = Some(format!("provider returned {}", response.status()));
                        }
                        Err(error) => last_error = Some(error.to_string()),
                    }
                }

                Err(ChannelError::FetchFailed(last_error.unwrap_or_else(|| {
                    "unknown TikTok cursor error".to_string()
                })))
            }
            #[cfg(test)]
            PageFetcher::Fixture(fixtures) => fixtures
                .get(&format!("tiktok:cursor:{}", request.cursor))
                .cloned()
                .ok_or_else(|| {
                    ChannelError::FetchFailed(format!(
                        "fixture not found: tiktok:cursor:{}",
                        request.cursor
                    ))
                }),
        }
    }

    async fn delay_between_pages(&self) {
        if matches!(self.fetcher, PageFetcher::Live(_)) {
            sleep(Duration::from_millis(250)).await;
        }
    }
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(350 * attempt as u64)
}

fn provider_for_url(source_url: &str) -> Option<Provider> {
    let host = host_from_url(source_url)?;
    let path = path_from_url(source_url).unwrap_or_default();

    if host == "youtube.com" || host.ends_with(".youtube.com") {
        if path.starts_with("/channel/")
            || path.starts_with("/c/")
            || path.starts_with("/user/")
            || path.starts_with("/@")
            || path.starts_with("/playlist")
        {
            return Some(Provider::YouTube);
        }
    }

    if (host == "tiktok.com" || host.ends_with(".tiktok.com")) && path.starts_with("/@") {
        return Some(Provider::TikTok);
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

    (!host.is_empty()).then_some(host)
}

fn path_from_url(source_url: &str) -> Option<String> {
    let without_scheme = source_url
        .strip_prefix("https://")
        .or_else(|| source_url.strip_prefix("http://"))?;
    let after_authority = without_scheme.split_once('/').map(|(_, path)| path)?;
    let path = format!(
        "/{}",
        after_authority.split(['?', '#']).next().unwrap_or("")
    );

    Some(path)
}

fn parse_youtube_initial(html: &str) -> Result<Value, ChannelError> {
    let marker = Regex::new(r#"ytInitialData\s*="#).unwrap();
    let start = marker
        .find(html)
        .map(|match_| match_.end())
        .ok_or(ChannelError::MissingJson("ytInitialData"))?;
    let json =
        extract_json_object_after(html, start).ok_or(ChannelError::MissingJson("ytInitialData"))?;

    serde_json::from_str(json).map_err(|error| ChannelError::InvalidJson(error.to_string()))
}

fn parse_youtube_api_key(html: &str) -> Option<String> {
    let config = parse_ytcfg(html)?;

    config
        .get("INNERTUBE_API_KEY")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn parse_youtube_context(html: &str) -> Option<Value> {
    parse_ytcfg(html)?.get("INNERTUBE_CONTEXT").cloned()
}

fn parse_ytcfg(html: &str) -> Option<Value> {
    let marker = Regex::new(r#"ytcfg\.set\s*\("#).unwrap();
    let start = marker.find(html)?.end();
    let json = extract_json_object_after(html, start)?;

    serde_json::from_str(json).ok()
}

fn default_youtube_context() -> Value {
    json!({
        "client": {
            "clientName": "WEB",
            "clientVersion": "2.20240501.00.00"
        }
    })
}

fn collect_youtube_page(
    value: &Value,
    videos: &mut Vec<ChannelVideo>,
    seen_videos: &mut HashSet<String>,
) -> Vec<String> {
    let mut continuations = Vec::new();

    collect_youtube_videos(value, videos, seen_videos);
    collect_youtube_continuations(value, &mut continuations);
    dedupe_strings(continuations)
}

fn collect_youtube_videos(
    value: &Value,
    videos: &mut Vec<ChannelVideo>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Object(object) => {
            for key in [
                "videoRenderer",
                "gridVideoRenderer",
                "playlistVideoRenderer",
            ] {
                if let Some(renderer) = object.get(key) {
                    if let Some(video) = youtube_video_from_renderer(renderer) {
                        if seen.insert(video.id.clone()) {
                            videos.push(video);
                        }
                    }
                }
            }

            for item in object.values() {
                collect_youtube_videos(item, videos, seen);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_youtube_videos(item, videos, seen);
            }
        }
        _ => {}
    }
}

fn youtube_video_from_renderer(renderer: &Value) -> Option<ChannelVideo> {
    let id = renderer
        .get("videoId")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            renderer
                .pointer("/navigationEndpoint/watchEndpoint/videoId")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })?;

    Some(ChannelVideo {
        id,
        title: renderer.get("title").and_then(text_from_value),
        thumbnail_url: best_thumbnail_url(renderer.get("thumbnail")?),
    })
}

fn collect_youtube_continuations(value: &Value, tokens: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(token) = object
                .get("continuationCommand")
                .and_then(|command| command.get("token"))
                .and_then(|token| token.as_str())
            {
                tokens.push(token.to_string());
            }

            if let Some(token) = object
                .get("nextContinuationData")
                .or_else(|| object.get("reloadContinuationData"))
                .and_then(|data| data.get("continuation"))
                .and_then(|token| token.as_str())
            {
                tokens.push(token.to_string());
            }

            for item in object.values() {
                collect_youtube_continuations(item, tokens);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_youtube_continuations(item, tokens);
            }
        }
        _ => {}
    }
}

fn parse_tiktok_state(html: &str) -> Result<Value, ChannelError> {
    if let Some(json) = script_json_by_id(html, "SIGI_STATE") {
        return serde_json::from_str(json)
            .map_err(|error| ChannelError::InvalidJson(error.to_string()));
    }

    if let Some(json) = script_json_by_id(html, "__NEXT_DATA__") {
        return serde_json::from_str(json)
            .map_err(|error| ChannelError::InvalidJson(error.to_string()));
    }

    Err(ChannelError::MissingJson("SIGI_STATE or __NEXT_DATA__"))
}

fn parse_tiktok_page(value: &Value) -> TikTokPage {
    let mut videos = Vec::new();
    let mut seen = HashSet::new();
    collect_tiktok_videos(value, &mut videos, &mut seen);

    TikTokPage {
        videos,
        cursor: find_cursor(value),
        has_more: find_has_more(value),
        sec_uid: find_key_string(value, "secUid"),
    }
}

fn collect_tiktok_videos(
    value: &Value,
    videos: &mut Vec<ChannelVideo>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Object(object) => {
            if let Some(video) = tiktok_video_from_item(value) {
                if seen.insert(video.id.clone()) {
                    videos.push(video);
                }
            }

            if let Some(item_module) = object
                .get("ItemModule")
                .and_then(|module| module.as_object())
            {
                for item in item_module.values() {
                    collect_tiktok_videos(item, videos, seen);
                }
            }

            if let Some(items) = object
                .get("itemList")
                .or_else(|| object.get("items"))
                .and_then(|items| items.as_array())
            {
                for item in items {
                    collect_tiktok_videos(item, videos, seen);
                }
            }

            for item in object.values() {
                collect_tiktok_videos(item, videos, seen);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_tiktok_videos(item, videos, seen);
            }
        }
        _ => {}
    }
}

fn tiktok_video_from_item(item: &Value) -> Option<ChannelVideo> {
    let object = item.as_object()?;
    let id = object
        .get("id")
        .or_else(|| object.get("itemId"))
        .or_else(|| object.get("aweme_id"))
        .and_then(first_string)?;

    if object.get("video").is_none() && object.get("video_id").is_none() {
        return None;
    }

    let thumbnail_url = object.get("video").and_then(|video| {
        video
            .get("cover")
            .or_else(|| video.get("originCover"))
            .or_else(|| video.get("dynamicCover"))
            .or_else(|| video.get("coverUrl"))
            .and_then(first_string)
    });

    Some(ChannelVideo {
        id,
        title: object
            .get("desc")
            .or_else(|| object.get("description"))
            .and_then(first_string),
        thumbnail_url,
    })
}

fn find_cursor(value: &Value) -> Option<String> {
    find_key_string(value, "cursor")
        .or_else(|| find_key_string(value, "maxCursor"))
        .or_else(|| find_key_string(value, "max_cursor"))
}

fn find_has_more(value: &Value) -> bool {
    find_key_bool(value, "hasMore")
        .or_else(|| find_key_bool(value, "has_more"))
        .unwrap_or(false)
}

fn find_key_string(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(found) = object.get(key).and_then(first_string) {
                return Some(found);
            }

            object.values().find_map(|item| find_key_string(item, key))
        }
        Value::Array(items) => items.iter().find_map(|item| find_key_string(item, key)),
        _ => None,
    }
}

fn find_key_bool(value: &Value, key: &str) -> Option<bool> {
    match value {
        Value::Object(object) => {
            if let Some(found) = object.get(key).and_then(|item| item.as_bool()) {
                return Some(found);
            }

            object.values().find_map(|item| find_key_bool(item, key))
        }
        Value::Array(items) => items.iter().find_map(|item| find_key_bool(item, key)),
        _ => None,
    }
}

fn push_unique_videos(
    videos: &mut Vec<ChannelVideo>,
    seen: &mut HashSet<String>,
    next: Vec<ChannelVideo>,
) {
    for video in next {
        if seen.insert(video.id.clone()) {
            videos.push(video);
        }
    }
}

fn text_from_value(value: &Value) -> Option<String> {
    value
        .get("simpleText")
        .and_then(|text| text.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            value
                .get("runs")
                .and_then(|runs| runs.as_array())
                .and_then(|runs| runs.iter().find_map(|run| run.get("text")))
                .and_then(|text| text.as_str())
                .map(ToOwned::to_owned)
        })
        .or_else(|| first_string(value))
}

fn best_thumbnail_url(value: &Value) -> Option<String> {
    value
        .get("thumbnails")
        .and_then(|items| items.as_array())
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find_map(|item| item.get("url").and_then(|url| url.as_str()))
        })
        .map(ToOwned::to_owned)
        .or_else(|| first_string(value))
}

fn first_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .as_array()
        .and_then(|items| items.iter().find_map(first_string))
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

fn dedupe_strings(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const YOUTUBE_CHANNEL_URL: &str = "https://www.youtube.com/@fixture/videos";
    const TIKTOK_PROFILE_URL: &str = "https://www.tiktok.com/@fixture";

    #[tokio::test]
    async fn youtube_channel_follows_continuations_until_complete() {
        let fetcher = ChannelFetcher::fixture([
            (
                YOUTUBE_CHANNEL_URL.to_string(),
                youtube_initial_html().to_string(),
            ),
            (
                "youtube:continuation:YT_CONT_1".to_string(),
                youtube_continuation_one().to_string(),
            ),
            (
                "youtube:continuation:YT_CONT_2".to_string(),
                youtube_continuation_two().to_string(),
            ),
        ]);

        let videos = fetcher.fetch_channel(YOUTUBE_CHANNEL_URL).await.unwrap();

        assert_eq!(videos.len(), 5);
        assert_eq!(videos[0].id, "yt001");
        assert_eq!(videos[4].id, "yt005");
        assert_eq!(videos[4].title.as_deref(), Some("YouTube five"));
    }

    #[tokio::test]
    async fn tiktok_profile_follows_cursor_until_complete() {
        let fetcher = ChannelFetcher::fixture([
            (
                TIKTOK_PROFILE_URL.to_string(),
                tiktok_initial_html().to_string(),
            ),
            (
                "tiktok:cursor:20".to_string(),
                tiktok_cursor_twenty().to_string(),
            ),
            (
                "tiktok:cursor:40".to_string(),
                tiktok_cursor_forty().to_string(),
            ),
        ]);

        let videos = fetcher.fetch_channel(TIKTOK_PROFILE_URL).await.unwrap();

        assert_eq!(videos.len(), 5);
        assert_eq!(videos[0].id, "tt001");
        assert_eq!(videos[4].id, "tt005");
        assert_eq!(videos[4].thumbnail_url.as_deref(), Some("https://tt/5.jpg"));
    }

    #[test]
    fn rejects_non_channel_urls() {
        assert_eq!(
            provider_for_url("https://www.youtube.com/watch?v=abc"),
            None
        );
        assert_eq!(
            provider_for_url("https://www.tiktok.com/@fixture"),
            Some(Provider::TikTok)
        );
        assert_eq!(
            provider_for_url("https://www.youtube.com/playlist?list=PL123"),
            Some(Provider::YouTube)
        );
    }

    pub(crate) fn youtube_initial_html() -> &'static str {
        r#"
          <html>
            <script>
              ytcfg.set({
                "INNERTUBE_API_KEY": "test-api-key",
                "INNERTUBE_CONTEXT": {"client": {"clientName": "WEB", "clientVersion": "test"}}
              });
              var ytInitialData = {
                "contents": [
                  {"videoRenderer": {
                    "videoId": "yt001",
                    "title": {"runs": [{"text": "YouTube one"}]},
                    "thumbnail": {"thumbnails": [{"url": "https://yt/1-small.jpg"}, {"url": "https://yt/1.jpg"}]}
                  }},
                  {"videoRenderer": {
                    "videoId": "yt002",
                    "title": {"simpleText": "YouTube two"},
                    "thumbnail": {"thumbnails": [{"url": "https://yt/2.jpg"}]}
                  }},
                  {"continuationItemRenderer": {
                    "continuationEndpoint": {"continuationCommand": {"token": "YT_CONT_1"}}
                  }}
                ]
              };
            </script>
          </html>
        "#
    }

    pub(crate) fn youtube_continuation_one() -> &'static str {
        r#"
          {
            "onResponseReceivedActions": [
              {"appendContinuationItemsAction": {"continuationItems": [
                {"richItemRenderer": {"content": {"videoRenderer": {
                  "videoId": "yt003",
                  "title": {"runs": [{"text": "YouTube three"}]},
                  "thumbnail": {"thumbnails": [{"url": "https://yt/3.jpg"}]}
                }}}},
                {"videoRenderer": {
                  "videoId": "yt004",
                  "title": {"runs": [{"text": "YouTube four"}]},
                  "thumbnail": {"thumbnails": [{"url": "https://yt/4.jpg"}]}
                }},
                {"continuationItemRenderer": {
                  "continuationEndpoint": {"continuationCommand": {"token": "YT_CONT_2"}}
                }}
              ]}}
            ]
          }
        "#
    }

    pub(crate) fn youtube_continuation_two() -> &'static str {
        r#"
          {
            "onResponseReceivedActions": [
              {"appendContinuationItemsAction": {"continuationItems": [
                {"gridVideoRenderer": {
                  "videoId": "yt005",
                  "title": {"simpleText": "YouTube five"},
                  "thumbnail": {"thumbnails": [{"url": "https://yt/5.jpg"}]}
                }}
              ]}}
            ]
          }
        "#
    }

    pub(crate) fn tiktok_initial_html() -> &'static str {
        r#"
          <html>
            <script id="SIGI_STATE" type="application/json">
              {
                "ItemModule": {
                  "tt001": {"id": "tt001", "desc": "TikTok one", "video": {"cover": "https://tt/1.jpg"}},
                  "tt002": {"id": "tt002", "desc": "TikTok two", "video": {"cover": "https://tt/2.jpg"}}
                },
                "UserModule": {"users": {"fixture": {"secUid": "SEC_UID"}}},
                "UserPage": {"cursor": "20", "hasMore": true}
              }
            </script>
          </html>
        "#
    }

    pub(crate) fn tiktok_cursor_twenty() -> &'static str {
        r#"
          {
            "itemList": [
              {"id": "tt003", "desc": "TikTok three", "video": {"cover": "https://tt/3.jpg"}},
              {"id": "tt004", "desc": "TikTok four", "video": {"cover": "https://tt/4.jpg"}}
            ],
            "max_cursor": "40",
            "has_more": true
          }
        "#
    }

    pub(crate) fn tiktok_cursor_forty() -> &'static str {
        r#"
          {
            "itemList": [
              {"id": "tt005", "desc": "TikTok five", "video": {"cover": "https://tt/5.jpg"}}
            ],
            "cursor": "60",
            "hasMore": false
          }
        "#
    }
}
