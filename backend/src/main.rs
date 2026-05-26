mod bulk;
mod channel;
mod extract;
mod model;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bulk::{BulkDownloadError, BulkDownloadRequest, BulkDownloader};
use channel::{ChannelError, ChannelFetcher};
use extract::{ExtractError, Extractor};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

const FRONTEND_ORIGIN: &str = "http://localhost:3000";
const HEALTH_MESSAGE: &str = "OK: backend is healthy";

#[derive(Clone)]
struct AppState {
    bulk_downloader: Arc<BulkDownloader>,
    channel_fetcher: Arc<ChannelFetcher>,
    extractor: Arc<Extractor>,
}

impl AppState {
    fn live() -> Result<Self, reqwest::Error> {
        let extractor = Arc::new(Extractor::live()?);

        Ok(Self {
            bulk_downloader: Arc::new(BulkDownloader::live(extractor.clone())?),
            channel_fetcher: Arc::new(ChannelFetcher::live()?),
            extractor,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ExtractQuery {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ChannelQuery {
    url: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

struct ApiError {
    status: axum::http::StatusCode,
    message: String,
}

impl From<ExtractError> for ApiError {
    fn from(error: ExtractError) -> Self {
        let status = match error {
            ExtractError::UnsupportedUrl => axum::http::StatusCode::BAD_REQUEST,
            ExtractError::FetchFailed(_) => axum::http::StatusCode::BAD_GATEWAY,
            ExtractError::MissingJson(_)
            | ExtractError::InvalidJson(_)
            | ExtractError::MissingField(_)
            | ExtractError::NoStreams => axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        };

        Self {
            status,
            message: error.message(),
        }
    }
}

impl From<ChannelError> for ApiError {
    fn from(error: ChannelError) -> Self {
        let status = match error {
            ChannelError::UnsupportedUrl => axum::http::StatusCode::BAD_REQUEST,
            ChannelError::FetchFailed(_) => axum::http::StatusCode::BAD_GATEWAY,
            ChannelError::MissingJson(_)
            | ChannelError::InvalidJson(_)
            | ChannelError::MissingField(_)
            | ChannelError::NoVideos => axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        };

        Self {
            status,
            message: error.message(),
        }
    }
}

impl From<BulkDownloadError> for ApiError {
    fn from(error: BulkDownloadError) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.message().to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/extract", get(extract))
        .route("/api/channel", get(channel))
        .route("/api/download/bulk", post(download_bulk))
        .with_state(state)
        .layer(cors_layer())
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(HeaderValue::from_static(FRONTEND_ORIGIN))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
}

async fn health() -> impl IntoResponse {
    HEALTH_MESSAGE
}

async fn extract(
    State(state): State<AppState>,
    Query(query): Query<ExtractQuery>,
) -> Result<Json<model::VideoInfo>, ApiError> {
    let info = state.extractor.extract(&query.url).await?;

    Ok(Json(info))
}

async fn channel(
    State(state): State<AppState>,
    Query(query): Query<ChannelQuery>,
) -> Result<Json<Vec<model::ChannelVideo>>, ApiError> {
    let videos = state.channel_fetcher.fetch_channel(&query.url).await?;

    Ok(Json(videos))
}

async fn download_bulk(
    State(state): State<AppState>,
    Json(request): Json<BulkDownloadRequest>,
) -> Result<Response, ApiError> {
    let stream = state.bulk_downloader.download_zip(request)?;
    let mut response = Body::from_stream(stream).into_response();
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"videos.zip\""),
    );

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState::live()?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("Backend listening on http://localhost:8080");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_route_returns_text_and_cors_header() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header(header::ORIGIN, FRONTEND_ORIGIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()
                .unwrap(),
            FRONTEND_ORIGIN
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), HEALTH_MESSAGE.as_bytes());
    }

    #[tokio::test]
    async fn extract_route_returns_normalized_youtube_metadata() {
        let source_url = "https://www.youtube.com/watch?v=abc123";
        let response = test_app_with_fixture(source_url, youtube_fixture())
            .oneshot(
                Request::builder()
                    .uri("/api/extract?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["platform"], "youtube");
        assert_eq!(json["id"], "abc123");
        assert_eq!(json["title"], "Fixture YouTube Video");
        assert_eq!(
            json["streams"][0]["url"],
            "https://video.example/itag18.mp4"
        );
    }

    #[tokio::test]
    async fn extract_route_returns_normalized_tiktok_metadata() {
        let source_url = "https://www.tiktok.com/@fixture/video/7222222222222222222";
        let response = test_app_with_fixture(source_url, tiktok_fixture())
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/extract?url=https%3A%2F%2Fwww.tiktok.com%2F%40fixture%2Fvideo%2F7222222222222222222",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["platform"], "tiktok");
        assert_eq!(json["id"], "7222222222222222222");
        assert_eq!(json["title"], "Fixture TikTok Video");
        assert_eq!(
            json["streams"][0]["url"],
            "https://v.example/no-watermark.mp4"
        );
        assert_eq!(json["streams"][0]["watermark"], false);
    }

    #[tokio::test]
    async fn extract_route_rejects_unsupported_hosts() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/extract?url=https%3A%2F%2Fexample.com%2Fvideo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    fn test_app() -> Router {
        app(AppState {
            bulk_downloader: Arc::new(BulkDownloader::fixture([])),
            channel_fetcher: Arc::new(ChannelFetcher::fixture([])),
            extractor: Arc::new(Extractor::fixture([])),
        })
    }

    fn test_app_with_fixture(source_url: &str, html: &'static str) -> Router {
        app(AppState {
            bulk_downloader: Arc::new(BulkDownloader::fixture([])),
            channel_fetcher: Arc::new(ChannelFetcher::fixture([])),
            extractor: Arc::new(Extractor::fixture([(
                source_url.to_string(),
                html.to_string(),
            )])),
        })
    }

    fn test_app_with_channel_fixtures(
        fixtures: impl IntoIterator<Item = (String, String)>,
    ) -> Router {
        app(AppState {
            bulk_downloader: Arc::new(BulkDownloader::fixture([])),
            channel_fetcher: Arc::new(ChannelFetcher::fixture(fixtures)),
            extractor: Arc::new(Extractor::fixture([])),
        })
    }

    fn test_app_with_download_fixtures(
        fixtures: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Router {
        app(AppState {
            bulk_downloader: Arc::new(BulkDownloader::fixture(fixtures)),
            channel_fetcher: Arc::new(ChannelFetcher::fixture([])),
            extractor: Arc::new(Extractor::fixture([])),
        })
    }

    fn youtube_fixture() -> &'static str {
        r#"
            <html>
              <script>
                var ytInitialPlayerResponse = {
                  "videoDetails": {
                    "videoId": "abc123",
                    "title": "Fixture YouTube Video",
                    "author": "Fixture Channel",
                    "lengthSeconds": "61",
                    "thumbnail": {
                      "thumbnails": [{"url": "https://img.example/large.jpg"}]
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
                        "audioQuality": "AUDIO_QUALITY_MEDIUM"
                      }
                    ]
                  }
                };
              </script>
            </html>
        "#
    }

    fn tiktok_fixture() -> &'static str {
        r#"
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
                        "downloadAddr": "https://v.example/watermark.mp4"
                      }
                    }
                  }
                }
              </script>
            </html>
        "#
    }

    #[tokio::test]
    async fn channel_route_returns_all_youtube_fixture_videos() {
        let response = test_app_with_channel_fixtures([
            (
                "https://www.youtube.com/@fixture/videos".to_string(),
                channel::tests::youtube_initial_html().to_string(),
            ),
            (
                "youtube:continuation:YT_CONT_1".to_string(),
                channel::tests::youtube_continuation_one().to_string(),
            ),
            (
                "youtube:continuation:YT_CONT_2".to_string(),
                channel::tests::youtube_continuation_two().to_string(),
            ),
        ])
        .oneshot(
            Request::builder()
                .uri("/api/channel?url=https%3A%2F%2Fwww.youtube.com%2F%40fixture%2Fvideos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.as_array().unwrap().len(), 5);
        assert_eq!(json[0]["id"], "yt001");
        assert_eq!(json[4]["id"], "yt005");
        assert_eq!(json[4]["title"], "YouTube five");
    }

    #[tokio::test]
    async fn channel_route_returns_all_tiktok_fixture_videos() {
        let response = test_app_with_channel_fixtures([
            (
                "https://www.tiktok.com/@fixture".to_string(),
                channel::tests::tiktok_initial_html().to_string(),
            ),
            (
                "tiktok:cursor:20".to_string(),
                channel::tests::tiktok_cursor_twenty().to_string(),
            ),
            (
                "tiktok:cursor:40".to_string(),
                channel::tests::tiktok_cursor_forty().to_string(),
            ),
        ])
        .oneshot(
            Request::builder()
                .uri("/api/channel?url=https%3A%2F%2Fwww.tiktok.com%2F%40fixture")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.as_array().unwrap().len(), 5);
        assert_eq!(json[0]["id"], "tt001");
        assert_eq!(json[4]["id"], "tt005");
        assert_eq!(json[4]["thumbnail_url"], "https://tt/5.jpg");
    }

    #[tokio::test]
    async fn bulk_download_route_streams_zip_archive() {
        let response = test_app_with_download_fixtures([
            ("yt001".to_string(), b"video-one".to_vec()),
            ("yt002".to_string(), b"video-two".to_vec()),
        ])
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/download/bulk")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"source_url":"https://www.youtube.com/@fixture/videos","ids":["yt001","yt002"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/zip"
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let entries = bulk::tests::read_stored_zip(&body);

        assert_eq!(entries.get("yt001.bin").unwrap(), b"video-one");
        assert_eq!(entries.get("yt002.bin").unwrap(), b"video-two");
    }
}
