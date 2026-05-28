use std::{
    collections::HashSet,
    io,
    io::Read,
    net::{IpAddr, Ipv6Addr},
    pin::Pin,
    process::{Command, Stdio},
    sync::Arc,
    task::{Context, Poll},
};

#[cfg(test)]
use std::collections::HashMap;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::{Client, RequestBuilder, Response};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::extract::Extractor;
use crate::model::{StreamInfo, VideoInfo};

const OUTPUT_CHANNEL_BUFFER: usize = 8;
const DOWNLOAD_CHANNEL_BUFFER: usize = 4;
const MAX_BULK_IDS: usize = 500;
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36";

#[derive(Debug, Deserialize)]
pub struct BulkDownloadRequest {
    pub source_url: Option<String>,
    pub cookie: Option<String>,
    pub format: Option<DownloadFormat>,
    pub quality: Option<String>,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadFormat {
    Mp4,
    Mp3,
}

#[derive(Debug)]
pub enum BulkDownloadError {
    EmptyIds,
    TooManyIds,
    InvalidOptions(String),
    PrepareFailed(String),
}

impl BulkDownloadError {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyIds => "ids must contain at least one video id".to_string(),
            Self::TooManyIds => "ids must contain at most 500 video ids".to_string(),
            Self::InvalidOptions(error) => error.clone(),
            Self::PrepareFailed(error) => error.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DownloadOptions {
    format: DownloadFormat,
    quality: DownloadQuality,
}

#[derive(Debug, Clone, Copy)]
enum DownloadQuality {
    Best,
    MaxHeight(u32),
}

#[derive(Clone)]
pub struct BulkDownloader {
    client: Client,
    extractor: Arc<Extractor>,
    source: DownloadSource,
}

#[derive(Clone)]
enum DownloadSource {
    Live,
    #[cfg(test)]
    Fixture(Arc<HashMap<String, Vec<u8>>>),
}

struct DownloadEntry {
    id: String,
    filename: String,
    receiver: mpsc::Receiver<Result<Bytes, String>>,
}

struct CentralDirectoryRecord {
    filename: Vec<u8>,
    crc32: u32,
    size: u32,
    local_header_offset: u32,
}

struct Crc32 {
    state: u32,
}

pub struct ReceiverStream {
    receiver: mpsc::Receiver<Result<Bytes, io::Error>>,
}

impl Stream for ReceiverStream {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

impl BulkDownloader {
    pub fn live(extractor: Arc<Extractor>) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder().user_agent(BROWSER_USER_AGENT).build()?,
            extractor,
            source: DownloadSource::Live,
        })
    }

    #[cfg(test)]
    pub fn fixture(items: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            client: Client::new(),
            extractor: Arc::new(Extractor::fixture([])),
            source: DownloadSource::Fixture(Arc::new(items.into_iter().collect())),
        }
    }

    pub async fn download_zip(
        &self,
        request: BulkDownloadRequest,
    ) -> Result<ReceiverStream, BulkDownloadError> {
        let options = DownloadOptions::from_request(&request)?;
        let ids = unique_ids(request.ids);

        if ids.is_empty() {
            return Err(BulkDownloadError::EmptyIds);
        }

        if ids.len() > MAX_BULK_IDS {
            return Err(BulkDownloadError::TooManyIds);
        }

        let entries = self
            .start_downloads(ids, request.source_url, request.cookie, options)
            .await?;
        let (sender, receiver) = mpsc::channel(OUTPUT_CHANNEL_BUFFER);

        tokio::spawn(async move {
            if let Err(error) = write_zip_stream(entries, sender.clone()).await {
                let _ = sender.send(Err(io::Error::other(error))).await;
            }
        });

        Ok(ReceiverStream { receiver })
    }

    async fn start_downloads(
        &self,
        ids: Vec<String>,
        source_url: Option<String>,
        cookie: Option<String>,
        options: DownloadOptions,
    ) -> Result<Vec<DownloadEntry>, BulkDownloadError> {
        if options.format == DownloadFormat::Mp3 {
            mp3_conversion_status().map_err(BulkDownloadError::PrepareFailed)?;
        }

        let mut entries = Vec::with_capacity(ids.len());

        for id in ids {
            let (sender, receiver) = mpsc::channel(DOWNLOAD_CHANNEL_BUFFER);

            match &self.source {
                DownloadSource::Live => {
                    let client = self.client.clone();
                    let extractor = self.extractor.clone();
                    let source_url = source_url.clone();
                    let cookie = cookie.clone();
                    let download_id = id.clone();
                    let resolved = resolve_download(
                        extractor.clone(),
                        source_url.as_deref(),
                        cookie.as_deref(),
                        &download_id,
                        options,
                    )
                    .await
                    .map_err(|error| {
                        BulkDownloadError::PrepareFailed(format!(
                            "failed to prepare {download_id}: {error}"
                        ))
                    })?;
                    if resolved.transcode_mp3 {
                        tokio::spawn(async move {
                            transcode_mp3_url(resolved.url, sender).await;
                        });
                    } else {
                        let response = open_download_response(
                            &client,
                            cookie.as_deref(),
                            &resolved.url,
                            &download_id,
                        )
                        .await
                        .map_err(BulkDownloadError::PrepareFailed)?;

                        tokio::spawn(async move {
                            stream_live_response(response, sender).await;
                        });
                    }

                    entries.push(DownloadEntry {
                        id,
                        filename: format!("{}.{}", safe_filename(&download_id), resolved.extension),
                        receiver,
                    });
                }
                #[cfg(test)]
                DownloadSource::Fixture(fixtures) => {
                    let bytes = fixtures.get(&id).cloned();
                    let missing_id = id.clone();

                    tokio::spawn(async move {
                        match bytes {
                            Some(bytes) => {
                                for chunk in bytes.chunks(8) {
                                    if sender
                                        .send(Ok(Bytes::copy_from_slice(chunk)))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            None => {
                                let _ = sender
                                    .send(Err(format!("fixture not found for id {missing_id}")))
                                    .await;
                            }
                        }
                    });

                    entries.push(DownloadEntry {
                        filename: format!("{}.bin", safe_filename(&id)),
                        id,
                        receiver,
                    });
                }
            }
        }

        Ok(entries)
    }
}

async fn stream_live_response(response: Response, sender: mpsc::Sender<Result<Bytes, String>>) {
    let result = async {
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;

            if sender.send(Ok(chunk)).await.is_err() {
                return Ok(());
            }
        }

        Ok(())
    }
    .await;

    if let Err(error) = result {
        let _ = sender.send(Err(error)).await;
    }
}

async fn transcode_mp3_url(download_url: String, sender: mpsc::Sender<Result<Bytes, String>>) {
    let output_sender = sender.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut child = Command::new("gst-launch-1.0")
            .arg("-q")
            .arg("souphttpsrc")
            .arg(format!("location={download_url}"))
            .arg(format!("user-agent={BROWSER_USER_AGENT}"))
            .arg("!")
            .arg("decodebin")
            .arg("!")
            .arg("audioconvert")
            .arg("!")
            .arg("audioresample")
            .arg("!")
            .arg("lamemp3enc")
            .arg("target=bitrate")
            .arg("bitrate=192")
            .arg("cbr=true")
            .arg("!")
            .arg("id3v2mux")
            .arg("!")
            .arg("fdsink")
            .arg("fd=1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start MP3 encoder: {error}"))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to open MP3 encoder stdout".to_string())?;
        let mut buffer = [0u8; 16 * 1024];

        loop {
            let count = stdout
                .read(&mut buffer)
                .map_err(|error| format!("failed to read MP3 encoder output: {error}"))?;

            if count == 0 {
                break;
            }

            if output_sender
                .blocking_send(Ok(Bytes::copy_from_slice(&buffer[..count])))
                .is_err()
            {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("failed to finish MP3 encoder: {error}"))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("MP3 encoder failed: {}", stderr.trim()))
        }
    })
    .await
    .map_err(|error| format!("MP3 encoder task failed: {error}"))
    .and_then(|result| result);

    if let Err(error) = result {
        let _ = sender.send(Err(error)).await;
    }
}

struct ResolvedDownload {
    url: String,
    extension: &'static str,
    transcode_mp3: bool,
}

async fn resolve_download(
    extractor: Arc<Extractor>,
    source_url: Option<&str>,
    cookie: Option<&str>,
    id: &str,
    options: DownloadOptions,
) -> Result<ResolvedDownload, String> {
    if id.starts_with("http://") || id.starts_with("https://") {
        return Ok(ResolvedDownload {
            url: id.to_string(),
            extension: options.extension(),
            transcode_mp3: options.format == DownloadFormat::Mp3,
        });
    }

    let source_url =
        source_url.ok_or_else(|| "source_url is required for provider ids".to_string())?;
    let video_url = if is_youtube_url(source_url) {
        format!("https://www.youtube.com/watch?v={id}")
    } else if is_tiktok_url(source_url) {
        let username = tiktok_username(source_url)
            .ok_or_else(|| "TikTok source_url must contain @username".to_string())?;
        format!("https://www.tiktok.com/{username}/video/{id}")
    } else if is_facebook_url(source_url) {
        format!("https://www.facebook.com/watch/?v={id}")
    } else {
        return Err("source_url must be YouTube, TikTok, or Facebook".to_string());
    };

    let info = extractor
        .extract_with_cookie(&video_url, cookie)
        .await
        .map_err(|error| error.message())?;
    let stream = select_stream(&info, options)?;

    Ok(ResolvedDownload {
        url: stream.url.clone(),
        extension: options.extension(),
        transcode_mp3: options.format == DownloadFormat::Mp3,
    })
}

async fn open_download_response(
    client: &Client,
    cookie: Option<&str>,
    download_url: &str,
    id: &str,
) -> Result<Response, String> {
    let media_client = if is_googlevideo_ipv6_url(download_url) {
        Client::builder()
            .user_agent(BROWSER_USER_AGENT)
            .local_address(IpAddr::V6(Ipv6Addr::UNSPECIFIED))
            .build()
            .unwrap_or_else(|_| client.clone())
    } else {
        client.clone()
    };
    let mut request = provider_download_headers(media_client.get(download_url), download_url);

    if let Some(cookie) = clean_cookie(cookie) {
        request = request.header(reqwest::header::COOKIE, cookie);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("failed to reach download host for {id}: {error}"))?;

    if !response.status().is_success() {
        Err(format!(
            "download source for {id} returned {}",
            response.status()
        ))
    } else {
        Ok(response)
    }
}

fn is_googlevideo_ipv6_url(download_url: &str) -> bool {
    download_url.contains("googlevideo.com")
        && download_url.contains("ip=")
        && download_url.contains("%3A")
}

fn provider_download_headers(request: RequestBuilder, download_url: &str) -> RequestBuilder {
    if download_url.contains("googlevideo.com") {
        return request
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::REFERER, "https://www.youtube.com/");
    }

    if download_url.contains("tiktokcdn")
        || download_url.contains("tiktokv")
        || download_url.contains("byteoversea")
    {
        return request
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::REFERER, "https://www.tiktok.com/");
    }

    request
}

async fn write_zip_stream(
    entries: Vec<DownloadEntry>,
    sender: mpsc::Sender<Result<Bytes, io::Error>>,
) -> Result<(), String> {
    let mut offset = 0u32;
    let mut central_directory = Vec::with_capacity(entries.len());

    for entry in entries {
        let filename = entry.filename.into_bytes();
        let local_header_offset = offset;
        let header = local_file_header(&filename)?;
        offset = send_bytes(&sender, &mut offset, header).await?;

        let mut crc = Crc32::new();
        let mut size = 0u32;
        let mut receiver = entry.receiver;

        while let Some(chunk) = receiver.recv().await {
            let chunk =
                chunk.map_err(|error| format!("failed to download {}: {error}", entry.id))?;
            crc.update(&chunk);
            size = size
                .checked_add(u32::try_from(chunk.len()).map_err(|_| "file is too large")?)
                .ok_or("file is too large")?;
            offset = send_bytes(&sender, &mut offset, chunk).await?;
        }

        let crc32 = crc.finalize();
        let descriptor = data_descriptor(crc32, size);
        offset = send_bytes(&sender, &mut offset, descriptor).await?;

        central_directory.push(CentralDirectoryRecord {
            filename,
            crc32,
            size,
            local_header_offset,
        });
    }

    let central_directory_offset = offset;

    for record in &central_directory {
        let header = central_directory_header(record)?;
        offset = send_bytes(&sender, &mut offset, header).await?;
    }

    let central_directory_size = offset
        .checked_sub(central_directory_offset)
        .ok_or("invalid central directory size")?;
    let eocd = end_of_central_directory(
        u16::try_from(central_directory.len()).map_err(|_| "too many zip entries")?,
        central_directory_size,
        central_directory_offset,
    );
    let _ = send_bytes(&sender, &mut offset, eocd).await?;

    Ok(())
}

async fn send_bytes(
    sender: &mpsc::Sender<Result<Bytes, io::Error>>,
    offset: &mut u32,
    bytes: impl Into<Bytes>,
) -> Result<u32, String> {
    let bytes = bytes.into();
    let next_offset = offset
        .checked_add(u32::try_from(bytes.len()).map_err(|_| "zip is too large")?)
        .ok_or("zip is too large")?;
    sender
        .send(Ok(bytes))
        .await
        .map_err(|_| "client disconnected".to_string())?;

    Ok(next_offset)
}

fn local_file_header(filename: &[u8]) -> Result<Vec<u8>, String> {
    let filename_len = u16::try_from(filename.len()).map_err(|_| "filename is too long")?;
    let mut bytes = Vec::with_capacity(30 + filename.len());

    write_u32(&mut bytes, 0x0403_4b50);
    write_u16(&mut bytes, 20);
    write_u16(&mut bytes, 0x0008);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_u16(&mut bytes, filename_len);
    write_u16(&mut bytes, 0);
    bytes.extend_from_slice(filename);

    Ok(bytes)
}

fn data_descriptor(crc32: u32, size: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);

    write_u32(&mut bytes, 0x0807_4b50);
    write_u32(&mut bytes, crc32);
    write_u32(&mut bytes, size);
    write_u32(&mut bytes, size);

    bytes
}

fn central_directory_header(record: &CentralDirectoryRecord) -> Result<Vec<u8>, String> {
    let filename_len = u16::try_from(record.filename.len()).map_err(|_| "filename is too long")?;
    let mut bytes = Vec::with_capacity(46 + record.filename.len());

    write_u32(&mut bytes, 0x0201_4b50);
    write_u16(&mut bytes, 20);
    write_u16(&mut bytes, 20);
    write_u16(&mut bytes, 0x0008);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u32(&mut bytes, record.crc32);
    write_u32(&mut bytes, record.size);
    write_u32(&mut bytes, record.size);
    write_u16(&mut bytes, filename_len);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, record.local_header_offset);
    bytes.extend_from_slice(&record.filename);

    Ok(bytes)
}

fn end_of_central_directory(
    entry_count: u16,
    directory_size: u32,
    directory_offset: u32,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(22);

    write_u32(&mut bytes, 0x0605_4b50);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, 0);
    write_u16(&mut bytes, entry_count);
    write_u16(&mut bytes, entry_count);
    write_u32(&mut bytes, directory_size);
    write_u32(&mut bytes, directory_offset);
    write_u16(&mut bytes, 0);

    bytes
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

impl Crc32 {
    fn new() -> Self {
        Self { state: 0xffff_ffff }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u32::from(*byte);

            for _ in 0..8 {
                if self.state & 1 == 1 {
                    self.state = (self.state >> 1) ^ 0xedb8_8320;
                } else {
                    self.state >>= 1;
                }
            }
        }
    }

    fn finalize(self) -> u32 {
        !self.state
    }
}

fn unique_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();

    ids.into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn safe_filename(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn is_youtube_url(source_url: &str) -> bool {
    host_from_url(source_url).is_some_and(|host| {
        host == "youtube.com" || host.ends_with(".youtube.com") || host == "youtu.be"
    })
}

fn is_tiktok_url(source_url: &str) -> bool {
    host_from_url(source_url)
        .is_some_and(|host| host == "tiktok.com" || host.ends_with(".tiktok.com"))
}

fn is_facebook_url(source_url: &str) -> bool {
    host_from_url(source_url).is_some_and(|host| {
        host == "facebook.com"
            || host.ends_with(".facebook.com")
            || host == "fb.watch"
            || host.ends_with(".fb.watch")
    })
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

fn tiktok_username(source_url: &str) -> Option<String> {
    source_url
        .split('/')
        .find(|part| part.starts_with('@'))
        .map(ToOwned::to_owned)
}

fn clean_cookie(cookie: Option<&str>) -> Option<&str> {
    cookie.map(str::trim).filter(|value| !value.is_empty())
}

impl DownloadOptions {
    fn from_request(request: &BulkDownloadRequest) -> Result<Self, BulkDownloadError> {
        Ok(Self {
            format: request.format.unwrap_or(DownloadFormat::Mp4),
            quality: DownloadQuality::from_request(request.quality.as_deref())
                .map_err(BulkDownloadError::InvalidOptions)?,
        })
    }

    fn extension(self) -> &'static str {
        match self.format {
            DownloadFormat::Mp4 => "mp4",
            DownloadFormat::Mp3 => "mp3",
        }
    }
}

impl DownloadQuality {
    fn from_request(raw: Option<&str>) -> Result<Self, String> {
        let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(Self::Best);
        };

        if raw.eq_ignore_ascii_case("best") {
            return Ok(Self::Best);
        }

        let height = raw
            .strip_suffix('p')
            .or_else(|| raw.strip_suffix('P'))
            .unwrap_or(raw)
            .parse::<u32>()
            .map_err(|_| "quality must be best or a height such as 720p".to_string())?;

        if height == 0 {
            return Err("quality height must be greater than zero".to_string());
        }

        Ok(Self::MaxHeight(height))
    }
}

fn select_stream(info: &VideoInfo, options: DownloadOptions) -> Result<&StreamInfo, String> {
    match options.format {
        DownloadFormat::Mp4 => select_mp4_stream(&info.streams, options.quality)
            .ok_or_else(|| "no MP4 video stream found".to_string()),
        DownloadFormat::Mp3 => select_mp3_source_stream(&info.streams)
            .ok_or_else(|| "no playable audio/video stream found for MP3 conversion".to_string()),
    }
}

fn select_mp4_stream(streams: &[StreamInfo], quality: DownloadQuality) -> Option<&StreamInfo> {
    let candidates: Vec<&StreamInfo> = streams
        .iter()
        .filter(|stream| {
            !stream.watermark
                && stream.has_video
                && stream
                    .mime_type
                    .as_deref()
                    .is_some_and(|mime| mime.starts_with("video/mp4"))
        })
        .collect();
    let with_audio: Vec<&StreamInfo> = candidates
        .iter()
        .copied()
        .filter(|stream| stream.has_audio)
        .collect();

    select_by_quality(&with_audio, quality).or_else(|| select_by_quality(&candidates, quality))
}

fn select_mp3_source_stream(streams: &[StreamInfo]) -> Option<&StreamInfo> {
    streams
        .iter()
        .filter(|stream| {
            !stream.watermark
                && stream.has_audio
                && stream.has_video
                && stream
                    .mime_type
                    .as_deref()
                    .is_some_and(|mime| mime.starts_with("video/mp4"))
        })
        .max_by_key(|stream| {
            (
                stream_height(stream).unwrap_or(0),
                stream.bitrate.unwrap_or(0),
            )
        })
        .or_else(|| {
            streams
                .iter()
                .filter(|stream| !stream.watermark && stream.has_audio && !stream.has_video)
                .max_by_key(|stream| stream.bitrate.unwrap_or(0))
        })
}

pub fn mp3_conversion_status() -> Result<(), String> {
    ensure_command_success("gst-launch-1.0", &["--version"])?;
    ensure_command_success("gst-inspect-1.0", &["lamemp3enc"])?;

    if ["avdec_aac", "faad", "fdkaacdec"]
        .iter()
        .any(|plugin| ensure_command_success("gst-inspect-1.0", &[*plugin]).is_ok())
    {
        Ok(())
    } else {
        Err("MP3 conversion requires a GStreamer AAC decoder plugin such as avdec_aac, faad, or fdkaacdec".to_string())
    }
}

fn ensure_command_success(command: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(command)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to run {command}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{command} {} failed", args.join(" ")))
    }
}

fn select_by_quality<'a>(
    streams: &[&'a StreamInfo],
    quality: DownloadQuality,
) -> Option<&'a StreamInfo> {
    match quality {
        DownloadQuality::Best => streams.iter().copied().max_by_key(|stream| {
            (
                stream_height(stream).unwrap_or(0),
                stream.bitrate.unwrap_or(0),
            )
        }),
        DownloadQuality::MaxHeight(target) => streams
            .iter()
            .copied()
            .filter(|stream| stream_height(stream).is_some_and(|height| height <= target))
            .max_by_key(|stream| {
                (
                    stream_height(stream).unwrap_or(0),
                    stream.bitrate.unwrap_or(0),
                )
            })
            .or_else(|| {
                streams.iter().copied().min_by_key(|stream| {
                    (
                        stream_height(stream).unwrap_or(u32::MAX),
                        stream.bitrate.unwrap_or(u64::MAX),
                    )
                })
            }),
    }
}

fn stream_height(stream: &StreamInfo) -> Option<u32> {
    stream.height.or_else(|| {
        stream.quality.as_deref().and_then(|quality| {
            let digits: String = quality
                .chars()
                .skip_while(|character| !character.is_ascii_digit())
                .take_while(|character| character.is_ascii_digit())
                .collect();

            digits.parse().ok()
        })
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[tokio::test]
    async fn streams_fixture_ids_as_zip_entries() {
        let downloader = BulkDownloader::fixture([
            ("alpha".to_string(), b"alpha bytes".to_vec()),
            ("beta".to_string(), b"beta bytes".to_vec()),
        ]);
        let stream = downloader
            .download_zip(BulkDownloadRequest {
                source_url: None,
                cookie: None,
                format: None,
                quality: None,
                ids: vec!["alpha".to_string(), "beta".to_string()],
            })
            .await
            .unwrap();
        let bytes = collect_stream(stream).await;
        let entries = read_stored_zip(&bytes);

        assert_eq!(entries.get("alpha.bin").unwrap(), b"alpha bytes");
        assert_eq!(entries.get("beta.bin").unwrap(), b"beta bytes");
    }

    #[test]
    fn removes_empty_and_duplicate_ids() {
        assert_eq!(
            unique_ids(vec![
                " alpha ".to_string(),
                "".to_string(),
                "alpha".to_string(),
                "beta".to_string()
            ]),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn selects_requested_mp4_quality_with_audio() {
        let streams = vec![
            stream(
                "https://video.example/360.mp4",
                "video/mp4",
                Some(360),
                true,
                true,
            ),
            stream(
                "https://video.example/720-video.mp4",
                "video/mp4",
                Some(720),
                false,
                true,
            ),
            stream(
                "https://video.example/480.mp4",
                "video/mp4",
                Some(480),
                true,
                true,
            ),
        ];
        let selected = select_mp4_stream(&streams, DownloadQuality::MaxHeight(720)).unwrap();

        assert_eq!(selected.url, "https://video.example/480.mp4");
    }

    #[test]
    fn selects_combined_video_source_for_mp3_conversion() {
        let streams = vec![
            stream(
                "https://audio.example/audio.m4a",
                "audio/mp4",
                None,
                true,
                false,
            ),
            stream(
                "https://video.example/video.mp4",
                "video/mp4",
                Some(720),
                true,
                true,
            ),
        ];
        let selected = select_mp3_source_stream(&streams).unwrap();

        assert_eq!(selected.url, "https://video.example/video.mp4");
    }

    fn stream(
        url: &str,
        mime_type: &str,
        height: Option<u32>,
        has_audio: bool,
        has_video: bool,
    ) -> StreamInfo {
        StreamInfo {
            url: url.to_string(),
            mime_type: Some(mime_type.to_string()),
            quality: height.map(|height| format!("{height}p")),
            width: None,
            height,
            bitrate: None,
            has_audio,
            has_video,
            watermark: false,
        }
    }

    pub(crate) async fn collect_stream(mut stream: ReceiverStream) -> Vec<u8> {
        let mut bytes = Vec::new();

        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
        }

        bytes
    }

    pub(crate) fn read_stored_zip(bytes: &[u8]) -> HashMap<String, Vec<u8>> {
        let eocd_offset = bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4b, 0x05, 0x06])
            .unwrap();
        let entry_count = read_u16(bytes, eocd_offset + 10) as usize;
        let central_offset = read_u32(bytes, eocd_offset + 16) as usize;
        let mut cursor = central_offset;
        let mut entries = HashMap::new();

        for _ in 0..entry_count {
            assert_eq!(read_u32(bytes, cursor), 0x0201_4b50);
            let size = read_u32(bytes, cursor + 20) as usize;
            let name_len = read_u16(bytes, cursor + 28) as usize;
            let extra_len = read_u16(bytes, cursor + 30) as usize;
            let comment_len = read_u16(bytes, cursor + 32) as usize;
            let local_offset = read_u32(bytes, cursor + 42) as usize;
            let name =
                String::from_utf8(bytes[cursor + 46..cursor + 46 + name_len].to_vec()).unwrap();
            let local_name_len = read_u16(bytes, local_offset + 26) as usize;
            let local_extra_len = read_u16(bytes, local_offset + 28) as usize;
            let data_start = local_offset + 30 + local_name_len + local_extra_len;
            let data_end = data_start + size;

            entries.insert(name, bytes[data_start..data_end].to_vec());
            cursor += 46 + name_len + extra_len + comment_len;
        }

        entries
    }

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }
}
