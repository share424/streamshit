use std::convert::Infallible;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};

use clap::Parser;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "streamshit")]
#[command(about = "A simple video streaming server")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "6969")]
    port: u16,

    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Directory containing video files
    #[arg(short, long, default_value = ".")]
    video_dir: String,
}

fn get_local_ip() -> Result<String, Box<dyn std::error::Error>> {
    // Connect to a remote address to determine local IP
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    let local_addr = socket.local_addr()?;
    Ok(local_addr.ip().to_string())
}

async fn list_videos_handler(
    video_dir: String,
    server_url: String,
    _req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let video_list = get_video_list(&video_dir);
    let html = generate_video_list_html(&video_list, &server_url);

    let response = Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(html)))
        .unwrap();

    Ok(response)
}

fn get_video_list(path: &str) -> Vec<PathBuf> {
    let video_extensions = ["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v"];
    let mut video_list = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        if video_extensions.contains(&ext_str.to_lowercase().as_str()) {
                            video_list.push(path);
                        }
                    }
                }
            }
        }
    }

    video_list.sort();
    video_list
}

fn generate_video_list_html(videos: &[PathBuf], server_url: &str) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Streamshit</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        h1 { color: #333; }
        .server-info { 
            background-color: #e7f3ff; 
            padding: 15px; 
            border-radius: 5px; 
            margin-bottom: 20px; 
        }
        .video-list { list-style-type: none; padding: 0; }
        .video-item { 
            margin: 10px 0; 
            padding: 15px; 
            background-color: #f5f5f5; 
            border-radius: 5px; 
        }
        .video-name { 
            font-weight: bold; 
            margin-bottom: 5px; 
        }
        .video-url { 
            font-size: 0.9em; 
            color: #666; 
            word-break: break-all; 
        }
        .video-item a { 
            text-decoration: none; 
            color: #007bff; 
        }
        .video-item a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>Streamshit</h1>
"#,
    );

    // Add server info
    html.push_str(&format!(
        "<div class=\"server-info\"><strong>Server URL:</strong> {}</div>",
        server_url
    ));

    if videos.is_empty() {
        html.push_str("<p>No video files found in the directory.</p>");
    } else {
        html.push_str("<ul class=\"video-list\">");
        for video in videos {
            if let Some(filename) = video.file_name() {
                if let Some(name) = filename.to_str() {
                    let full_url = format!("{}/{}", server_url, name);
                    html.push_str(&format!(
                        r#"<li class="video-item">
                            <div class="video-name">{}</div>
                            <div class="video-url"><a href="{}" target="_blank">{}</a></div>
                        </li>"#,
                        name, full_url, full_url
                    ));
                }
            }
        }
        html.push_str("</ul>");
    }

    html.push_str("</body></html>");
    html
}

async fn router(
    video_dir: String,
    server_url: String,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let method = req.method();

    match (method, path) {
        (&Method::GET, "/") => {
            // Serve the video list page
            list_videos_handler(video_dir, server_url, req).await
        }
        (&Method::GET, path) => {
            // Remove leading slash and check if it's a video file
            let filename = path.strip_prefix('/').unwrap_or(path);

            // Check if this is a video file by checking if it exists and has a video extension
            if is_video_file(&video_dir, filename) {
                serve_video(video_dir, filename).await
            } else {
                // 404 for any other path
                let response = Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/html")
                    .body(Full::new(Bytes::from("<h1>404 Not Found</h1>")))
                    .unwrap();
                Ok(response)
            }
        }
        _ => {
            // 404 for any other path
            let response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/html")
                .body(Full::new(Bytes::from("<h1>404 Not Found</h1>")))
                .unwrap();
            Ok(response)
        }
    }
}

async fn serve_video(
    video_dir: String,
    filename: &str,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let video_path = Path::new(&video_dir).join(filename);

    // Security check: prevent directory traversal
    if !video_path.starts_with(&video_dir) {
        let response = Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "text/html")
            .body(Full::new(Bytes::from("<h1>403 Forbidden</h1>")))
            .unwrap();
        return Ok(response);
    }

    match fs::read(&video_path) {
        Ok(content) => {
            let mime_type = get_mime_type(filename);
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime_type)
                .header("Accept-Ranges", "bytes")
                .header("Cache-Control", "public, max-age=3600")
                .body(Full::new(Bytes::from(content)))
                .unwrap();
            Ok(response)
        }
        Err(_) => {
            let response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/html")
                .body(Full::new(Bytes::from("<h1>404 Video Not Found</h1>")))
                .unwrap();
            Ok(response)
        }
    }
}

fn get_mime_type(filename: &str) -> &'static str {
    let extension = Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase());

    match extension.as_deref() {
        Some("mp4") => "video/mp4",
        Some("avi") => "video/x-msvideo",
        Some("mkv") => "video/x-matroska",
        Some("mov") => "video/quicktime",
        Some("wmv") => "video/x-ms-wmv",
        Some("flv") => "video/x-flv",
        Some("webm") => "video/webm",
        Some("m4v") => "video/x-m4v",
        _ => "application/octet-stream",
    }
}

fn is_video_file(video_dir: &str, filename: &str) -> bool {
    let video_extensions = ["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v"];

    // Check if file has video extension
    let has_video_extension = Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| video_extensions.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false);

    if !has_video_extension {
        return false;
    }

    // Check if file actually exists in the video directory
    let video_path = Path::new(video_dir).join(filename);
    video_path.exists() && video_path.is_file()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    // Get the actual local IP address
    let local_ip = get_local_ip().unwrap_or_else(|_| "localhost".to_string());
    let server_url = format!("http://{}:{}", local_ip, args.port);

    println!("Starting video server on {}", addr);
    println!("Video directory: {}", args.video_dir);
    println!("Server URL: {}", server_url);

    // We create a TcpListener and bind it to 127.0.0.1:3000
    let listener = TcpListener::bind(addr).await?;
    let video_dir = args.video_dir.clone();

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);
        let video_dir_clone = video_dir.clone();
        let server_url_clone = server_url.clone();

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            let service = service_fn(move |req| {
                router(video_dir_clone.clone(), server_url_clone.clone(), req)
            });
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
