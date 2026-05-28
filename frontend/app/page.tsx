"use client";

import { FormEvent, useEffect, useMemo, useState } from "react";

const apiBaseUrl =
  process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080";

type ChannelVideo = {
  id: string;
  title: string | null;
  thumbnail_url: string | null;
};

type VideoInfo = {
  id: string | null;
  title: string | null;
  thumbnail_url: string | null;
};

type DownloadFormat = "mp4" | "mp3";

type Capabilities = {
  formats: DownloadFormat[];
  mp3_available: boolean;
  mp3_error: string | null;
};

type SaveFilePicker = (options: {
  suggestedName: string;
  types: Array<{
    description: string;
    accept: Record<string, string[]>;
  }>;
}) => Promise<{
  createWritable: () => Promise<WritableStream<Uint8Array>>;
}>;

export default function Home() {
  const [sourceUrl, setSourceUrl] = useState("");
  const [cookie, setCookie] = useState("");
  const [videos, setVideos] = useState<ChannelVideo[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [downloadFormat, setDownloadFormat] = useState<DownloadFormat>("mp4");
  const [downloadQuality, setDownloadQuality] = useState("best");
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [status, setStatus] = useState("Ready");
  const [isFetching, setIsFetching] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);

  const allSelected = videos.length > 0 && selectedIds.size === videos.length;
  const selectedCount = selectedIds.size;
  const normalizedApiBaseUrl = useMemo(
    () => apiBaseUrl.replace(/\/$/, ""),
    [],
  );
  const mp3Available = capabilities?.mp3_available ?? false;

  useEffect(() => {
    let ignore = false;

    async function loadCapabilities() {
      try {
        const nextCapabilities =
          await fetchCapabilities(normalizedApiBaseUrl);

        if (ignore) {
          return;
        }

        setCapabilities(nextCapabilities);

        if (!nextCapabilities.mp3_available) {
          setDownloadFormat("mp4");
        }
      } catch {
        if (!ignore) {
          setCapabilities({
            formats: ["mp4"],
            mp3_available: false,
            mp3_error: "Could not read backend download capabilities.",
          });
          setDownloadFormat("mp4");
        }
      }
    }

    loadCapabilities();

    return () => {
      ignore = true;
    };
  }, [normalizedApiBaseUrl]);

  async function fetchVideos(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const url = sourceUrl.trim();

    if (!url) {
      setStatus("Enter a channel, playlist, or profile URL.");
      return;
    }

    setIsFetching(true);
    setStatus("Fetching videos...");

    try {
      const params = new URLSearchParams({ url });
      const cleanCookie = cookie.trim();

      if (cleanCookie) {
        params.set("cookie", cleanCookie);
      }

      const items = isSingleVideoUrl(url)
        ? await fetchSingleVideo(normalizedApiBaseUrl, params)
        : await fetchChannelVideos(normalizedApiBaseUrl, params);

      setVideos(items);
      setSelectedIds(new Set(items.map((item) => item.id)));
      setStatus(
        items.length === 1 ? "Loaded 1 video." : `Loaded ${items.length} videos.`,
      );
    } catch (error) {
      setVideos([]);
      setSelectedIds(new Set());
      setStatus(error instanceof Error ? error.message : "Fetch failed.");
    } finally {
      setIsFetching(false);
    }
  }

  async function downloadSelected() {
    const ids = Array.from(selectedIds);

    if (ids.length === 0) {
      setStatus("Select at least one video.");
      return;
    }

    if (downloadFormat === "mp3" && !mp3Available) {
      setStatus(capabilities?.mp3_error ?? "MP3 downloads are not available.");
      return;
    }

    setIsDownloading(true);
    setStatus(`Preparing ${ids.length} videos...`);

    try {
      const response = await fetch(`${normalizedApiBaseUrl}/api/download/bulk`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          source_url: sourceUrl.trim(),
          cookie: cookie.trim() || undefined,
          format: downloadFormat,
          quality: downloadQuality,
          ids,
        }),
      });

      if (!response.ok) {
        throw new Error(await readApiError(response));
      }

      await saveZip(response);
      setStatus(`Downloaded ${ids.length} videos.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Download failed.");
    } finally {
      setIsDownloading(false);
    }
  }

  function toggleAll() {
    setSelectedIds((current) => {
      if (videos.length > 0 && current.size === videos.length) {
        return new Set();
      }

      return new Set(videos.map((item) => item.id));
    });
  }

  function toggleOne(id: string) {
    setSelectedIds((current) => {
      const next = new Set(current);

      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }

      return next;
    });
  }

  return (
    <main className="app-shell">
      <section className="workspace" aria-labelledby="bulk-title">
        <header className="page-header">
          <div>
            <p className="eyebrow">Video Downloader</p>
            <h1 id="bulk-title">Bulk Download</h1>
          </div>
          <span className="status-pill">{status}</span>
        </header>

        <form className="url-form" onSubmit={fetchVideos}>
          <label htmlFor="source-url">Link</label>
          <div className="input-row">
            <input
              id="source-url"
              name="source-url"
              placeholder="https://www.youtube.com/@channel/videos"
              value={sourceUrl}
              onChange={(event) => setSourceUrl(event.target.value)}
            />
            <button type="submit" disabled={isFetching || isDownloading}>
              {isFetching ? "Fetching..." : "Fetch"}
            </button>
          </div>
          <label htmlFor="provider-cookie">Cookie</label>
          <textarea
            id="provider-cookie"
            name="provider-cookie"
            placeholder="c_user=...; xs=..."
            value={cookie}
            onChange={(event) => setCookie(event.target.value)}
          />
        </form>

        <div className="toolbar">
          <label className="select-all">
            <input
              type="checkbox"
              checked={allSelected}
              disabled={videos.length === 0 || isDownloading}
              onChange={toggleAll}
            />
            <span>Chọn tất cả</span>
          </label>
          <div className="download-options">
            <label htmlFor="download-format">
              <span>Format</span>
              <select
                id="download-format"
                value={downloadFormat}
                disabled={isDownloading}
                onChange={(event) =>
                  setDownloadFormat(event.target.value as DownloadFormat)
                }
              >
                <option value="mp4">MP4</option>
                <option value="mp3" disabled={!mp3Available}>
                  MP3
                </option>
              </select>
            </label>
            <label htmlFor="download-quality">
              <span>Quality</span>
              <select
                id="download-quality"
                value={downloadQuality}
                disabled={isDownloading || downloadFormat === "mp3"}
                onChange={(event) => setDownloadQuality(event.target.value)}
              >
                <option value="best">Best</option>
                <option value="1080p">1080p</option>
                <option value="720p">720p</option>
                <option value="480p">480p</option>
                <option value="360p">360p</option>
                <option value="240p">240p</option>
              </select>
            </label>
          </div>
          <button
            type="button"
            className="download-button"
            disabled={selectedCount === 0 || isFetching || isDownloading}
            onClick={downloadSelected}
          >
            {isDownloading
              ? "Downloading..."
              : `Tải xuống ${selectedCount} video`}
          </button>
        </div>

        <div className="video-list" role="list">
          {videos.map((video) => (
            <label className="video-row" key={video.id} role="listitem">
              <input
                type="checkbox"
                checked={selectedIds.has(video.id)}
                disabled={isDownloading}
                onChange={() => toggleOne(video.id)}
              />
              <div className="thumb" aria-hidden="true">
                {video.thumbnail_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img src={video.thumbnail_url} alt="" />
                ) : (
                  <span>{video.id.slice(0, 2).toUpperCase()}</span>
                )}
              </div>
              <div className="video-meta">
                <strong>{video.title ?? video.id}</strong>
                <span>{video.id}</span>
              </div>
            </label>
          ))}
        </div>
      </section>
    </main>
  );
}

async function fetchChannelVideos(apiBaseUrl: string, params: URLSearchParams) {
  const response = await fetch(`${apiBaseUrl}/api/channel?${params.toString()}`);

  if (!response.ok) {
    throw new Error(await readApiError(response));
  }

  return (await response.json()) as ChannelVideo[];
}

async function fetchCapabilities(apiBaseUrl: string) {
  const response = await fetch(`${apiBaseUrl}/api/capabilities`);

  if (!response.ok) {
    throw new Error(await readApiError(response));
  }

  return (await response.json()) as Capabilities;
}

async function fetchSingleVideo(apiBaseUrl: string, params: URLSearchParams) {
  const response = await fetch(`${apiBaseUrl}/api/extract?${params.toString()}`);

  if (!response.ok) {
    throw new Error(await readApiError(response));
  }

  const video = (await response.json()) as VideoInfo;

  if (!video.id) {
    throw new Error("Could not find a video id in this URL.");
  }

  return [
    {
      id: video.id,
      title: video.title,
      thumbnail_url: video.thumbnail_url,
    },
  ];
}

function isSingleVideoUrl(value: string) {
  try {
    const url = new URL(value);
    const host = url.hostname.toLowerCase();
    const path = url.pathname;

    if (host === "youtu.be") {
      return path.length > 1;
    }

    if (host === "youtube.com" || host.endsWith(".youtube.com")) {
      return url.searchParams.has("v") || path.startsWith("/shorts/");
    }

    if (host === "tiktok.com" || host.endsWith(".tiktok.com")) {
      return /\/@[^/]+\/video\/\d+/.test(path);
    }

    if (
      host === "facebook.com" ||
      host.endsWith(".facebook.com") ||
      host === "fb.watch" ||
      host.endsWith(".fb.watch")
    ) {
      return (
        url.searchParams.has("v") ||
        path.startsWith("/reel/") ||
        /\/videos\/[^/]+/.test(path)
      );
    }
  } catch {
    return false;
  }

  return false;
}

async function readApiError(response: Response) {
  try {
    const body = (await response.json()) as { error?: string };

    return body.error ?? `${response.status} ${response.statusText}`;
  } catch {
    return `${response.status} ${response.statusText}`;
  }
}

async function saveZip(response: Response) {
  const filename = filenameFromDisposition(
    response.headers.get("content-disposition"),
  );
  const picker = (window as Window & { showSaveFilePicker?: SaveFilePicker })
    .showSaveFilePicker;

  if (picker && response.body) {
    const handle = await picker({
      suggestedName: filename,
      types: [
        {
          description: "ZIP archive",
          accept: {
            "application/zip": [".zip"],
          },
        },
      ],
    });
    const writable = await handle.createWritable();
    await response.body.pipeTo(writable);
    return;
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement("a");

  anchor.href = objectUrl;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(objectUrl);
}

function filenameFromDisposition(disposition: string | null) {
  const match = disposition?.match(/filename="?([^";]+)"?/i);

  return match?.[1] ?? "videos.zip";
}
