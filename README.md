# 🎬 StreamShit

A lightweight, fast video streaming server built in Rust. Stream your local video collection over the network with a simple web interface. I primarily use this to stream my movies to my TV/Phone using VLC.

## ✨ Features

- 🚀 **Fast & Lightweight**: Built with Rust and Hyper for maximum performance
- 🌐 **Network Streaming**: Access your videos from any device on your network
- 🎥 **Multiple Formats**: Supports MP4, AVI, MKV, MOV, WMV, FLV, WebM, M4V
- ⚡ **Direct Streaming**: Videos accessible directly at `ip:port/filename.ext`
- 🔍 **Auto Discovery**: Automatically detects and displays your local IP
- 📁 **Flexible Directory**: Point to any directory containing your videos

## 🛠️ Installation

### Prerequisites
- Rust (latest stable version)
- Cargo (comes with Rust)

### Build from Source
```bash
git clone https://github.com/share424/streamshit.git
cd streamshit
cargo build --release
```

## 🚀 Usage

### Basic Usage
```bash
# Serve videos from current directory on default port (6969)
cargo run

# Serve videos from specific directory
cargo run -- --video-dir /path/to/your/movies

# Use custom port
cargo run -- --port 8080

# Bind to specific host
cargo run -- --host 192.168.1.100 --port 3000
```

## 📄 License

MIT