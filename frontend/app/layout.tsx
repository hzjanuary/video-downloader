import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Video Downloader",
  description: "Video downloader platform health check",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
