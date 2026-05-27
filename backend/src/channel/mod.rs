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
    Facebook,
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
    referer: Option<String>,
    cookie: Option<String>,
}

#[derive(Debug, Clone)]
struct TikTokPage {
    videos: Vec<ChannelVideo>,
    cursor: Option<String>,
    has_more: bool,
    sec_uid: Option<String>,
}

#[derive(Debug, Clone)]
struct FacebookCursor {
    source_url: String,
    next_url: Option<String>,
    cursor: Option<String>,
    cookie: Option<String>,
}

#[derive(Debug, Clone)]
struct FacebookPage {
    videos: Vec<ChannelVideo>,
    next_url: Option<String>,
    cursor: Option<String>,
    has_more: bool,
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

    #[allow(dead_code)]
    pub async fn fetch_channel(&self, source_url: &str) -> Result<Vec<ChannelVideo>, ChannelError> {
        self.fetch_channel_with_cookie(source_url, None).await
    }

    pub async fn fetch_channel_with_cookie(
        &self,
        source_url: &str,
        cookie: Option<&str>,
    ) -> Result<Vec<ChannelVideo>, ChannelError> {
        match provider_for_url(source_url).ok_or(ChannelError::UnsupportedUrl)? {
            Provider::YouTube => self.fetch_youtube_channel(source_url).await,
            Provider::TikTok => self.fetch_tiktok_profile(source_url, cookie).await,
            Provider::Facebook => self.fetch_facebook_collection(source_url, cookie).await,
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
        cookie: Option<&str>,
    ) -> Result<Vec<ChannelVideo>, ChannelError> {
        let html = self.fetch_get_with_cookie(source_url, cookie).await?;
        let initial_json = parse_tiktok_state(&html)?;
        let initial_page = parse_tiktok_page(&initial_json);
        let initial_video_count = initial_page.videos.len();
        let initial_sec_uid = initial_page.sec_uid.clone();
        let initial_cursor = if initial_page.has_more {
            initial_page.cursor.clone()
        } else if initial_video_count == 0 && initial_sec_uid.is_some() {
            Some(
                initial_page
                    .cursor
                    .clone()
                    .unwrap_or_else(|| "0".to_string()),
            )
        } else {
            None
        };
        let mut videos = Vec::new();
        let mut seen_videos = HashSet::new();

        push_unique_videos(&mut videos, &mut seen_videos, initial_page.videos);

        let mut cursor = initial_cursor.map(|cursor| TikTokCursor {
            sec_uid: initial_sec_uid,
            cursor,
            referer: Some(source_url.to_string()),
            cookie: cookie.map(ToOwned::to_owned),
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
                    referer: request.referer.clone(),
                    cookie: request.cookie.clone(),
                });
            pages += 1;
        }

        if videos.is_empty() {
            return Err(ChannelError::NoVideos);
        }

        Ok(videos)
    }

    async fn fetch_facebook_collection(
        &self,
        source_url: &str,
        cookie: Option<&str>,
    ) -> Result<Vec<ChannelVideo>, ChannelError> {
        let html = self.fetch_get_with_cookie(source_url, cookie).await?;
        let initial_page = parse_facebook_page(&html);
        let mut videos = Vec::new();
        let mut seen_videos = HashSet::new();

        push_unique_videos(&mut videos, &mut seen_videos, initial_page.videos);

        let mut cursor = initial_page
            .has_more
            .then_some(FacebookCursor {
                source_url: source_url.to_string(),
                next_url: initial_page.next_url,
                cursor: initial_page.cursor,
                cookie: cookie.map(ToOwned::to_owned),
            })
            .filter(|request| request.next_url.is_some() || request.cursor.is_some());
        let mut seen_cursors = HashSet::new();
        let mut pages = 1usize;

        while let Some(request) = cursor.take() {
            if pages >= MAX_PAGES {
                break;
            }

            let cursor_key = request
                .next_url
                .clone()
                .or_else(|| request.cursor.clone())
                .unwrap_or_default();

            if !seen_cursors.insert(cursor_key) {
                break;
            }

            self.delay_between_pages().await;
            let page = self.fetch_facebook_cursor(&request).await?;
            let parsed = parse_facebook_page(&page);
            push_unique_videos(&mut videos, &mut seen_videos, parsed.videos);

            cursor = parsed
                .has_more
                .then_some(FacebookCursor {
                    source_url: request.source_url,
                    next_url: parsed.next_url,
                    cursor: parsed.cursor,
                    cookie: request.cookie,
                })
                .filter(|next| next.next_url.is_some() || next.cursor.is_some());
            pages += 1;
        }

        if videos.is_empty() {
            return Err(ChannelError::NoVideos);
        }

        Ok(videos)
    }

    async fn fetch_get(&self, url: &str) -> Result<String, ChannelError> {
        self.fetch_get_with_cookie(url, None).await
    }

    async fn fetch_get_with_cookie(
        &self,
        url: &str,
        cookie: Option<&str>,
    ) -> Result<String, ChannelError> {
        match &self.fetcher {
            PageFetcher::Live(client) => {
                let mut last_error = None;

                for attempt in 0..RETRIES {
                    if attempt > 0 {
                        sleep(retry_delay(attempt)).await;
                    }

                    let mut request = client.get(url);

                    if let Some(cookie) = clean_cookie(cookie) {
                        request = request.header(reqwest::header::COOKIE, cookie);
                    }

                    match request.send().await {
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

    async fn fetch_facebook_cursor(
        &self,
        request: &FacebookCursor,
    ) -> Result<String, ChannelError> {
        match &self.fetcher {
            PageFetcher::Live(_) => {
                let endpoint = request
                    .next_url
                    .clone()
                    .or_else(|| {
                        request
                            .cursor
                            .as_deref()
                            .map(|cursor| append_query_param(&request.source_url, "cursor", cursor))
                    })
                    .ok_or(ChannelError::MissingField("facebook cursor"))?;

                self.fetch_get_with_cookie(&endpoint, request.cookie.as_deref())
                    .await
            }
            #[cfg(test)]
            PageFetcher::Fixture(fixtures) => {
                if let Some(next_url) = &request.next_url {
                    return fixtures.get(next_url).cloned().ok_or_else(|| {
                        ChannelError::FetchFailed(format!("fixture not found: {next_url}"))
                    });
                }

                let cursor = request
                    .cursor
                    .as_deref()
                    .ok_or(ChannelError::MissingField("facebook cursor"))?;

                fixtures
                    .get(&format!("facebook:cursor:{cursor}"))
                    .cloned()
                    .ok_or_else(|| {
                        ChannelError::FetchFailed(format!(
                            "fixture not found: facebook:cursor:{cursor}"
                        ))
                    })
            }
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
                let verify_fp = request
                    .cookie
                    .as_deref()
                    .and_then(|cookie| cookie_value(cookie, "s_v_web_id"));
                let endpoint = if let Some(verify_fp) = verify_fp.as_deref() {
                    format!(
                        "https://m.tiktok.com/api/post/item_list/?aid=1988&cookie_enabled=true&count=35&verifyFp={}&secUid={}&cursor={}",
                        percent_encode(verify_fp),
                        sec_uid,
                        request.cursor
                    )
                } else {
                    format!(
                        "https://www.tiktok.com/api/post/item_list/?aid=1988&app_language=en&app_name=tiktok_web&browser_language=en-US&browser_name=Mozilla&browser_online=true&browser_platform=Win32&browser_version=5.0&channel=tiktok_web&count=35&cursor={}&device_platform=web_pc&focus_state=true&from_page=user&history_len=2&is_fullscreen=false&is_page_visible=true&language=en&region=US&screen_height=1080&screen_width=1920&secUid={}&tz_name=UTC&user_is_login=false&webcast_language=en",
                        request.cursor, sec_uid
                    )
                };
                let mut last_error = None;

                for attempt in 0..RETRIES {
                    if attempt > 0 {
                        sleep(retry_delay(attempt)).await;
                    }

                    let referer = request
                        .referer
                        .as_deref()
                        .unwrap_or("https://www.tiktok.com/");

                    let mut builder = client
                        .get(&endpoint)
                        .header("accept", "application/json, text/plain, */*")
                        .header("accept-language", "en-US,en;q=0.9")
                        .header("referer", referer);

                    if let Some(cookie) = clean_cookie(request.cookie.as_deref()) {
                        builder = builder.header(reqwest::header::COOKIE, cookie);
                    }

                    match builder.send().await {
                        Ok(response) if response.status().is_success() => {
                            let text = response
                                .text()
                                .await
                                .map_err(|error| ChannelError::FetchFailed(error.to_string()))?;

                            if text.trim().is_empty() {
                                return Err(ChannelError::FetchFailed(
                                    "TikTok returned an empty cursor response; paste a fresh TikTok browser cookie including s_v_web_id if the profile is verification-gated"
                                        .to_string(),
                                ));
                            }

                            return Ok(text);
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

    if host == "fb.watch"
        || host.ends_with(".fb.watch")
        || host == "facebook.com"
        || host.ends_with(".facebook.com")
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

    let config = marker.find_iter(html).find_map(|match_| {
        let start = skip_ascii_whitespace(html, match_.end());

        if html.as_bytes().get(start) != Some(&b'{') {
            return None;
        }

        let json = extract_json_object_after(html, start)?;
        serde_json::from_str(json).ok()
    });

    config
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

            if let Some(lockup) = object.get("lockupViewModel") {
                if let Some(video) = youtube_video_from_lockup(lockup) {
                    if seen.insert(video.id.clone()) {
                        videos.push(video);
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
        thumbnail_url: renderer.get("thumbnail").and_then(best_thumbnail_url),
    })
}

fn youtube_video_from_lockup(lockup: &Value) -> Option<ChannelVideo> {
    let id = lockup
        .get("contentId")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            lockup
                .pointer(
                    "/rendererContext/commandContext/onTap/innertubeCommand/watchEndpoint/videoId",
                )
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })?;

    let title = lockup
        .pointer("/metadata/lockupMetadataViewModel/title/content")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            lockup
                .pointer("/metadata/lockupMetadataViewModel/title")
                .and_then(text_from_value)
        });

    let thumbnail_url = lockup
        .pointer("/contentImage/thumbnailViewModel/image")
        .and_then(best_thumbnail_url)
        .or_else(|| lockup.get("contentImage").and_then(best_thumbnail_url));

    Some(ChannelVideo {
        id,
        title,
        thumbnail_url,
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

    if let Some(json) = script_json_by_id(html, "__UNIVERSAL_DATA_FOR_REHYDRATION__") {
        return serde_json::from_str(json)
            .map_err(|error| ChannelError::InvalidJson(error.to_string()));
    }

    Err(ChannelError::MissingJson(
        "SIGI_STATE, __NEXT_DATA__, or __UNIVERSAL_DATA_FOR_REHYDRATION__",
    ))
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

fn parse_facebook_page(html: &str) -> FacebookPage {
    let decoded_html = decode_html_entities(html);
    let mut videos = Vec::new();
    let mut seen = HashSet::new();

    collect_facebook_video_links(&decoded_html, &mut videos, &mut seen);

    let values = parse_json_scripts(&decoded_html);
    let mut cursor = None;
    let mut has_more = false;

    for value in &values {
        collect_facebook_videos_from_json(value, &mut videos, &mut seen);

        if cursor.is_none() {
            cursor = find_facebook_cursor(value);
        }

        has_more = has_more || find_facebook_has_more(value);
    }

    let next_url = find_facebook_next_url(&decoded_html);
    has_more = has_more || next_url.is_some();

    FacebookPage {
        videos,
        next_url,
        cursor,
        has_more,
    }
}

fn collect_facebook_video_links(
    html: &str,
    videos: &mut Vec<ChannelVideo>,
    seen: &mut HashSet<String>,
) {
    for pattern in [
        r#"(?:https?://(?:www\.|m\.|web\.)?facebook\.com)?/(?:watch/\?v=|reel/)(\d{5,})"#,
        r#"(?:https?://(?:www\.|m\.|web\.)?facebook\.com)?/[^/"'<>?]+/videos/(\d{5,})"#,
        r#"[?&](?:v|video_id)=(\d{5,})"#,
        r#""(?:videoID|video_id|videoId)"\s*:\s*"?(\d{5,})"?"#,
    ] {
        let regex = Regex::new(pattern).unwrap();

        for captures in regex.captures_iter(html) {
            let Some(id) = captures.get(1).map(|match_| match_.as_str().to_string()) else {
                continue;
            };

            if seen.insert(id.clone()) {
                videos.push(ChannelVideo {
                    id,
                    title: None,
                    thumbnail_url: None,
                });
            }
        }
    }
}

fn collect_facebook_videos_from_json(
    value: &Value,
    videos: &mut Vec<ChannelVideo>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Object(object) => {
            if let Some(video) = facebook_video_from_object(value) {
                if let Some(existing) = videos.iter_mut().find(|item| item.id == video.id) {
                    if existing.title.is_none() {
                        existing.title = video.title;
                    }
                    if existing.thumbnail_url.is_none() {
                        existing.thumbnail_url = video.thumbnail_url;
                    }
                } else if seen.insert(video.id.clone()) {
                    videos.push(video);
                }
            }

            for item in object.values() {
                collect_facebook_videos_from_json(item, videos, seen);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_facebook_videos_from_json(item, videos, seen);
            }
        }
        _ => {}
    }
}

fn facebook_video_from_object(value: &Value) -> Option<ChannelVideo> {
    let object = value.as_object()?;
    let id = object
        .get("videoID")
        .or_else(|| object.get("video_id"))
        .or_else(|| object.get("videoId"))
        .or_else(|| object.get("id"))
        .and_then(first_string)
        .filter(|id| is_numeric_video_id(id))?;

    if !is_facebook_video_object(value) {
        return None;
    }

    let title = object
        .get("title")
        .or_else(|| object.get("name"))
        .or_else(|| object.get("message"))
        .or_else(|| object.get("description"))
        .and_then(first_string);
    let thumbnail_url = object
        .get("thumbnail_url")
        .or_else(|| object.get("thumbnailUrl"))
        .or_else(|| object.get("picture"))
        .or_else(|| object.get("image"))
        .and_then(first_string)
        .or_else(|| {
            value
                .pointer("/thumbnail/uri")
                .or_else(|| value.pointer("/thumbnail/image/uri"))
                .or_else(|| value.pointer("/preferred_thumbnail/image/uri"))
                .or_else(|| value.pointer("/image/uri"))
                .and_then(first_string)
        });

    Some(ChannelVideo {
        id,
        title,
        thumbnail_url,
    })
}

fn is_facebook_video_object(value: &Value) -> bool {
    let typename = value
        .get("__typename")
        .or_else(|| value.get("__isNode"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if typename.contains("video") || typename.contains("reel") {
        return true;
    }

    for key in [
        "playable_url",
        "playable_url_quality_hd",
        "browser_native_hd_url",
        "browser_native_sd_url",
        "video",
    ] {
        if value.get(key).is_some() {
            return true;
        }
    }

    ["url", "permalink_url", "permalinkUrl"]
        .into_iter()
        .any(|key| {
            value
                .get(key)
                .and_then(first_string)
                .is_some_and(|url| is_facebook_video_url(&url))
        })
}

fn find_facebook_next_url(html: &str) -> Option<String> {
    let href_pattern = Regex::new(r#"(?is)<a[^>]+href=["']([^"']+)["'][^>]*>"#).unwrap();

    let next_url = href_pattern
        .captures_iter(html)
        .filter_map(|captures| captures.get(1))
        .map(|match_| decode_html_entities(match_.as_str()))
        .find(|href| {
            href.contains("cursor=")
                || href.contains("after=")
                || href.contains("pagination")
                || href.contains("page_info")
        })
        .map(|href| absolute_facebook_url(&href));

    next_url
}

fn find_facebook_cursor(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if object
                .get("has_next_page")
                .or_else(|| object.get("hasNextPage"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                if let Some(cursor) = object
                    .get("end_cursor")
                    .or_else(|| object.get("endCursor"))
                    .or_else(|| object.get("cursor"))
                    .and_then(first_string)
                {
                    return Some(cursor);
                }
            }

            object.values().find_map(find_facebook_cursor)
        }
        Value::Array(items) => items.iter().find_map(find_facebook_cursor),
        _ => None,
    }
}

fn find_facebook_has_more(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            if object
                .get("has_next_page")
                .or_else(|| object.get("hasNextPage"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return true;
            }

            object.values().any(find_facebook_has_more)
        }
        Value::Array(items) => items.iter().any(find_facebook_has_more),
        _ => false,
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
    ["thumbnails", "sources"]
        .into_iter()
        .find_map(|key| {
            value
                .get(key)
                .and_then(|items| items.as_array())
                .and_then(|items| {
                    items
                        .iter()
                        .rev()
                        .find_map(|item| item.get("url").and_then(|url| url.as_str()))
                })
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            value
                .get("url")
                .and_then(|url| url.as_str())
                .map(ToOwned::to_owned)
        })
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

fn is_numeric_video_id(id: &str) -> bool {
    id.len() >= 5 && id.chars().all(|character| character.is_ascii_digit())
}

fn is_facebook_video_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();

    lower.contains("facebook.com/watch")
        || lower.contains("/videos/")
        || lower.contains("/reel/")
        || lower.contains("video_id=")
        || lower.contains("?v=")
}

fn absolute_facebook_url(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    if href.starts_with("//") {
        return format!("https:{href}");
    }

    if href.starts_with('/') {
        return format!("https://www.facebook.com{href}");
    }

    format!("https://www.facebook.com/{href}")
}

fn append_query_param(url: &str, key: &str, value: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };

    format!("{url}{separator}{key}={}", percent_encode(value))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

fn clean_cookie(cookie: Option<&str>) -> Option<&str> {
    cookie.map(str::trim).filter(|value| !value.is_empty())
}

fn cookie_value(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.trim().to_string())
    })
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#034;", "\"")
        .replace("&#34;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
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

fn skip_ascii_whitespace(text: &str, start_index: usize) -> usize {
    text.as_bytes()
        .iter()
        .enumerate()
        .skip(start_index)
        .find_map(|(index, byte)| (!byte.is_ascii_whitespace()).then_some(index))
        .unwrap_or(text.len())
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
    const FACEBOOK_PAGE_URL: &str = "https://www.facebook.com/fixture/videos";

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

    #[tokio::test]
    async fn youtube_channel_reads_lockup_view_model_tiles() {
        let fetcher = ChannelFetcher::fixture([(
            YOUTUBE_CHANNEL_URL.to_string(),
            youtube_lockup_html().to_string(),
        )]);

        let videos = fetcher.fetch_channel(YOUTUBE_CHANNEL_URL).await.unwrap();

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "lock001");
        assert_eq!(videos[0].title.as_deref(), Some("YouTube lockup"));
        assert_eq!(
            videos[0].thumbnail_url.as_deref(),
            Some("https://yt/lock-large.jpg")
        );
    }

    #[tokio::test]
    async fn tiktok_universal_hydration_uses_zero_cursor_when_feed_is_empty() {
        let fetcher = ChannelFetcher::fixture([
            (
                TIKTOK_PROFILE_URL.to_string(),
                tiktok_universal_empty_html().to_string(),
            ),
            (
                "tiktok:cursor:0".to_string(),
                tiktok_cursor_zero().to_string(),
            ),
        ]);

        let videos = fetcher.fetch_channel(TIKTOK_PROFILE_URL).await.unwrap();

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "tt000");
        assert_eq!(videos[0].title.as_deref(), Some("TikTok zero"));
    }

    #[tokio::test]
    async fn facebook_page_follows_cursor_until_complete() {
        let fetcher = ChannelFetcher::fixture([
            (
                FACEBOOK_PAGE_URL.to_string(),
                facebook_initial_html().to_string(),
            ),
            (
                "facebook:cursor:FB_CURSOR_1".to_string(),
                facebook_cursor_one().to_string(),
            ),
        ]);

        let videos = fetcher.fetch_channel(FACEBOOK_PAGE_URL).await.unwrap();

        assert_eq!(videos.len(), 3);
        assert_eq!(videos[0].id, "111111111111111");
        assert_eq!(videos[0].title.as_deref(), Some("Facebook one"));
        assert_eq!(videos[2].id, "333333333333333");
        assert_eq!(videos[2].thumbnail_url.as_deref(), Some("https://fb/3.jpg"));
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
        assert_eq!(
            provider_for_url("https://www.facebook.com/fixture/videos"),
            Some(Provider::Facebook)
        );
    }

    #[test]
    fn parses_ytcfg_object_after_scalar_setter() {
        let html = r#"
          <script>window.ytcfg.set('EMERGENCY_BASE_URL', '/error_204');</script>
          <script>ytcfg.set({"INNERTUBE_API_KEY": "key-after-scalar"});</script>
        "#;

        assert_eq!(
            parse_youtube_api_key(html).as_deref(),
            Some("key-after-scalar")
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

    pub(crate) fn youtube_lockup_html() -> &'static str {
        r#"
          <html>
            <script>
              var ytInitialData = {
                "contents": [
                  {"richItemRenderer": {"content": {"lockupViewModel": {
                    "contentId": "lock001",
                    "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
                    "contentImage": {"thumbnailViewModel": {"image": {"sources": [
                      {"url": "https://yt/lock-small.jpg", "width": 168, "height": 94},
                      {"url": "https://yt/lock-large.jpg", "width": 336, "height": 188}
                    ]}}},
                    "metadata": {"lockupMetadataViewModel": {"title": {"content": "YouTube lockup"}}},
                    "rendererContext": {"commandContext": {"onTap": {"innertubeCommand": {
                      "watchEndpoint": {"videoId": "lock001"}
                    }}}}
                  }}}}
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

    pub(crate) fn tiktok_universal_empty_html() -> &'static str {
        r#"
          <html>
            <script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" type="application/json">
              {
                "__DEFAULT_SCOPE__": {
                  "webapp.user-detail": {
                    "userInfo": {
                      "user": {"secUid": "SEC_UID", "uniqueId": "fixture"},
                      "itemList": []
                    }
                  }
                }
              }
            </script>
          </html>
        "#
    }

    pub(crate) fn tiktok_cursor_zero() -> &'static str {
        r#"
          {
            "itemList": [
              {"id": "tt000", "desc": "TikTok zero", "video": {"cover": "https://tt/0.jpg"}}
            ],
            "cursor": "20",
            "hasMore": false
          }
        "#
    }

    pub(crate) fn facebook_initial_html() -> &'static str {
        r#"
          <html>
            <script type="application/json">
              {
                "data": {
                  "page": {
                    "videos": {
                      "edges": [
                        {"node": {
                          "__typename": "Video",
                          "id": "111111111111111",
                          "title": "Facebook one",
                          "thumbnail": {"uri": "https://fb/1.jpg"},
                          "url": "https://www.facebook.com/watch/?v=111111111111111"
                        }},
                        {"node": {
                          "__typename": "Reel",
                          "id": "222222222222222",
                          "description": "Facebook two",
                          "preferred_thumbnail": {"image": {"uri": "https://fb/2.jpg"}},
                          "permalink_url": "https://www.facebook.com/reel/222222222222222"
                        }}
                      ],
                      "page_info": {
                        "has_next_page": true,
                        "end_cursor": "FB_CURSOR_1"
                      }
                    }
                  }
                }
              }
            </script>
          </html>
        "#
    }

    pub(crate) fn facebook_cursor_one() -> &'static str {
        r#"
          <html>
            <script type="application/json">
              {
                "data": {
                  "page": {
                    "videos": {
                      "edges": [
                        {"node": {
                          "__typename": "Video",
                          "id": "333333333333333",
                          "name": "Facebook three",
                          "thumbnail_url": "https://fb/3.jpg",
                          "url": "https://www.facebook.com/watch/?v=333333333333333"
                        }}
                      ],
                      "page_info": {
                        "has_next_page": false,
                        "end_cursor": "FB_CURSOR_2"
                      }
                    }
                  }
                }
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
