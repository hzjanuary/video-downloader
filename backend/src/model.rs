use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "youtube")]
    YouTube,
    #[serde(rename = "tiktok")]
    TikTok,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub mime_type: Option<String>,
    pub quality: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bitrate: Option<u64>,
    pub has_audio: bool,
    pub has_video: bool,
    pub watermark: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoInfo {
    pub platform: Platform,
    pub source_url: String,
    pub id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub duration_seconds: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub streams: Vec<StreamInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelVideo {
    pub id: String,
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
}
