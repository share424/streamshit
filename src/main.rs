use std::convert::Infallible;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
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

#[derive(Clone)]
struct VideoEntry {
    path: PathBuf,
    alias: String,
}

fn get_local_ip() -> Result<String, Box<dyn std::error::Error>> {
    // Connect to a remote address to determine local IP
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    let local_addr = socket.local_addr()?;
    Ok(local_addr.ip().to_string())
}

async fn list_videos_handler(
    video_list: Arc<Vec<VideoEntry>>,
    server_url: Arc<String>,
    _req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let html = generate_video_list_html(&video_list, &server_url);

    let response = Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(html)))
        .unwrap();

    Ok(response)
}

fn get_video_list(path: &str) -> Vec<VideoEntry> {
    let video_extensions = ["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v"];
    let mut video_paths = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        if video_extensions.contains(&ext_str.to_lowercase().as_str()) {
                            video_paths.push(path);
                        }
                    }
                }
            }
        }
    }
    video_paths.sort();

    video_paths
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            let alias = format!("{}.{}", i + 1, extension);
            VideoEntry { path, alias }
        })
        .collect()
}

fn generate_video_list_html(videos: &[VideoEntry], server_url: &str) -> String {
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
            if let Some(filename) = video.path.file_name() {
                if let Some(name) = filename.to_str() {
                    let full_url = format!("{}/{}", server_url, video.alias);
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
    req: Request<Incoming>,
    video_list: Arc<Vec<VideoEntry>>,
    server_url: Arc<String>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let method = req.method();

    match (method, path) {
        (&Method::GET, "/") => list_videos_handler(video_list, server_url, req).await,
        (&Method::GET, path) => {
            let filename = path.strip_prefix('/').unwrap_or(path);

            // Find video by alias or by filename
            let video_entry = video_list.iter().find(|v| {
                v.alias == filename || v.path.file_name().unwrap().to_str().unwrap() == filename
            });

            if let Some(entry) = video_entry {
                serve_video(&entry.path).await
            } else {
                not_found()
            }
        }
        _ => not_found(),
    }
}

async fn serve_video(video_path: &Path) -> Result<Response<Full<Bytes>>, Infallible> {
    match fs::read(video_path) {
        Ok(content) => {
            let mime_type = get_mime_type(video_path.to_str().unwrap());
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

fn not_found() -> Result<Response<Full<Bytes>>, Infallible> {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html")
        .body(Full::new(Bytes::from("<h1>404 Not Found</h1>")))
        .unwrap();
    Ok(response)
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let local_ip = get_local_ip().unwrap_or_else(|_| "localhost".to_string());
    let server_url = Arc::new(format!("http://{}:{}", local_ip, args.port));

    println!("Starting video server on {}", addr);
    println!("Video directory: {}", args.video_dir);
    println!("Server URL: {}", server_url);

    let video_list = Arc::new(get_video_list(&args.video_dir));
    println!("Found {} video files.", video_list.len());

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let video_list_clone = video_list.clone();
        let server_url_clone = server_url.clone();

        tokio::task::spawn(async move {
            let service = service_fn(move |req| {
                router(req, video_list_clone.clone(), server_url_clone.clone())
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
