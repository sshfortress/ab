Rust 压测工具 (Rust Load Testing Tool)
一个用 Rust 编写的简单而高效的压测工具，支持 HTTP (GET, POST, PUT, DELETE 等) 和 WebSocket 协议。它能够模拟并发用户请求，收集并展示关键性能指标，如响应时间、吞吐量和错误详情。

✨ 主要特性
多协议支持: 同时支持 HTTP/HTTPS 请求 (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS) 和 WebSocket 连接。

并发控制: 可自定义的并发用户数，模拟真实场景下的负载。

请求/连接数量控制: 精确控制总请求数或 WebSocket 连接的持续时间。

自定义请求参数:

HTTP: 支持传入自定义请求体 (如 JSON)、自定义 HTTP Header。

WebSocket: 支持连接建立后发送一条指定消息，并可设置连接持续时间。

详细性能报告: 输出请求/连接的总耗时、成功/失败次数、每秒请求数 (RPS) 以及延迟统计 (平均、最小、最大、P50, P90, P95, P99 百分位数)。

HTTP 状态码分布: 针对 HTTP 压测，提供详细的状态码统计。

错误信息汇总: 统计并显示各类错误及其发生次数。

🛠️ 构建项目
要构建此压测工具，你需要安装 Rust 编程语言及其工具链。如果你还没有安装，可以通过 rustup 进行安装。

克隆仓库 (如果适用):

Bash

git clone https://github.com/sshfortress/ab # 替换为你的仓库地址
cd ab
或者如果你是手动创建项目，请确保你的项目结构如下：

rust_ab_websocket/
├── Cargo.toml
└── src/
    └── main.rs
配置 Cargo.toml:
确保你的 Cargo.toml 文件包含以下依赖项：

Ini, TOML

# Cargo.toml
[package]
name = "rust_ab_websocket"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "blocking", "multipart"] }
tokio-tungstenite = { version = "0.23", features = ["native-tls"] }
url = "2.5"
clap = { version = "4", features = ["derive"] }
futures-util = "0.3"
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
hdrhistogram = "7.5"
构建:
在项目根目录运行以下命令来编译优化后的二进制文件：

Bash

cargo build --release
编译成功后，可执行文件将位于 target/release/rust_ab_websocket (Windows 上是 target/release/rust_ab_websocket.exe)。

🚀 使用方式
该工具通过命令行参数进行配置。

基本用法
Bash

./target/release/rust_ab_websocket [OPTIONS]
命令行参数
-c, --concurrency <CONCURRENCY>: 并发用户数 (默认: 1)。

-r, --requests <REQUESTS>: 总请求数 (HTTP) 或 WebSocket 并发连接数 (WebSocket 持续模式下)。 (默认: 1)。

-u, --url <URL>: 请求目标 URL (例如: http://localhost:8080/api 或 ws://echo.websocket.events)。

-m, --method <METHOD>: 请求方法 (例如: GET, POST, DELETE, WS。默认: GET)。

-d, --data <DATA>: HTTP 请求体 (仅适用于 POST, PUT, PATCH 等方法)。

例如: -d '{"key": "value"}'

-H, --headers <KEY:VALUE>: 自定义 HTTP Header (可重复使用)。

例如: -H "Content-Type: application/json" -H "Authorization: Bearer my_token"

--ws-message <WS_MESSAGE>: WebSocket 连接建立后发送的消息 (仅适用于 WS 方法)。

--ws-duration <WS_DURATION>: WebSocket 连接持续时间 (秒)。如果设置此参数，--requests 将表示并发的 WebSocket 连接数，而不是总消息数。

-t, --timeout <TIMEOUT>: 请求超时时间 (秒)。 (默认: 30)。

使用示例
1. HTTP GET 请求
对 http://httpbin.org/get 发送 1000 个 GET 请求，并发数为 10。

Bash

./target/release/rust_ab_websocket -c 10 -r 1000 -u "http://httpbin.org/get" -m GET


2. HTTP POST 请求 (带 JSON 数据和自定义 Header)
对 http://httpbin.org/post 发送 500 个 POST 请求，并发数为 5，并发送 JSON 数据和自定义 Header。

Bash

./target/release/rust_ab_websocket -c 5 -r 500 -u "http://httpbin.org/post" -m POST -d '{"username": "rust_user", "score": 99}' -H "Content-Type: application/json" -H "X-API-Key: abcdef123"


3. HTTP DELETE 请求
对 http://localhost:8080/resource/123 发送 10 个 DELETE 请求，并发数为 2。

Bash

./target/release/rust_ab_websocket -c 2 -r 10 -u "http://localhost:8080/resource/123" -m DELETE



4. WebSocket 连接 (发送一条消息)
建立 100 个 WebSocket 连接，并发数为 2，每个连接发送一次 "Hello WebSocket!" 消息。

Bash

./target/release/rust_ab_websocket -c 2 -r 100 -u "ws://echo.websocket.events" -m WS --ws-message "Hello WebSocket!"
echo.websocket.events 是一个公共的 WebSocket 回显服务器，用于测试。

5. WebSocket 持续连接 (指定持续时间)
建立 5 个并发 WebSocket 连接，每个连接保持 10 秒钟。

Bash

./target/release/rust_ab_websocket -c 5 -u "ws://echo.websocket.events" -m WS --ws-duration 10


报告解读
工具运行结束后会输出详细的压测报告：

总持续时间: 压测从开始到结束的总时间。

成功/失败请求/连接数: 完成的请求/连接总数及其成功或失败的分类。

每秒请求数 (RPS): 工具每秒能够处理的请求或连接数，衡量吞吐量。

延迟统计:

平均延迟: 所有成功请求的平均响应时间。

最小/最大延迟: 最快和最慢的响应时间。

百分位数 (P50, P90, P95, P99): 重要的延迟指标。例如，P99 为 100ms 意味着 99% 的请求在 100ms 内完成。

HTTP 状态码分布: (仅 HTTP 压测) 显示所有 HTTP 响应状态码 (如 200, 404, 500) 及其出现次数。

错误详情: 列出所有发生的错误类型及其计数，帮助你快速定位问题。
