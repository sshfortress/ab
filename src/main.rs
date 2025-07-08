// src/main.rs

use clap::Parser;
use reqwest::{Client, Method, StatusCode};
use std::collections::HashMap;
use std::time::{Instant, Duration};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::SinkExt; // 仅保留 SinkExt，因为 StreamExt 未被直接使用
use hdrhistogram::Histogram;
use url::Url; // 引入 url crate

/// 一个简单的 Rust 压测工具，支持 HTTP 和 WebSocket 协议。
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 并发用户数
    #[arg(short, long, default_value_t = 1)]
    concurrency: usize,

    /// 总请求数 (HTTP) 或 WebSocket 连接数
    #[arg(short, long, default_value_t = 1)] // 默认值设为1，避免ws_duration未指定时无请求
    requests: usize,

    /// 请求的URL (支持 http(s):// 和 ws(s)://)
    #[arg(short, long)]
    url: String,

    /// 请求方法 (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS) 或 'WS' 用于 WebSocket
    #[arg(short, long, default_value = "GET")]
    method: String,

    /// HTTP请求体 (仅适用于 POST/PUT/PATCH), 可以是字符串或JSON字符串
    #[arg(short = 'd', long)]
    data: Option<String>,

    /// 自定义HTTP Header (格式: "Key:Value"), 可重复使用
    #[arg(short = 'H', long, value_parser = parse_header, action = clap::ArgAction::Append)]
    headers: Vec<(String, String)>,

    /// WebSocket发送的消息 (可选，连接建立后发送一次)
    #[arg(long)]
    ws_message: Option<String>,

    /// WebSocket持续连接时间 (秒)。如果设置，将忽略 --requests 参数对WS连接次数的限制，
    /// 而是让每个WS连接持续指定时间。此模式下，--requests 表示并发的WS连接数。
    #[arg(long)]
    ws_duration: Option<u64>,

    /// 请求超时时间 (秒), 默认为 30 秒
    #[arg(short, long, default_value_t = 30)]
    timeout: u64,
}

/// 解析 "Key:Value" 格式的 Header 字符串
fn parse_header(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() == 2 {
        Ok((parts[0].trim().to_string(), parts[1].trim().to_string()))
    } else {
        Err(format!("无效的Header格式: \"{}\". 期望格式为 \"Key:Value\".", s))
    }
}

/// 单次请求的结果
#[derive(Debug)]
struct RequestResult {
    duration: Duration,
    success: bool,
    status_code: Option<StatusCode>, // HTTP 请求会填充，WebSocket 请求为 None
    error: Option<String>,
}

/// 执行 HTTP 请求
async fn make_http_request(
    client: &Client,
    method_str: &str,
    url: &str,
    data: Option<&str>,
    headers: &HashMap<String, String>,
) -> RequestResult {
    let start = Instant::now();
    let method = match method_str.to_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        _ => {
            return RequestResult {
                duration: start.elapsed(),
                success: false,
                status_code: None,
                error: Some(format!("不支持的HTTP方法: {}", method_str)),
            };
        }
    };

    let mut request_builder = client.request(method, url);

    if let Some(body) = data {
        request_builder = request_builder.body(body.to_string());
    }

    for (key, value) in headers {
        request_builder = request_builder.header(key, value);
    }

    match request_builder.send().await {
        Ok(response) => {
            let status = response.status();
            let success = status.is_success();
            let duration = start.elapsed();
            // 确保读取响应体，以便连接被完全消耗和关闭
            let _ = response.bytes().await;

            RequestResult {
                duration,
                success,
                status_code: Some(status), // 填充 HTTP 状态码
                error: if success { None } else { Some(format!("HTTP Status: {}", status)) },
            }
        }
        Err(e) => RequestResult {
            duration: start.elapsed(),
            success: false,
            status_code: None, // 连接失败，没有 HTTP 状态码
            error: Some(e.to_string()),
        },
    }
}

/// 执行 WebSocket 请求
async fn make_websocket_request(
    url_str: &str,
    message: Option<&str>,
    duration_secs: Option<u64>,
) -> RequestResult {
    let start = Instant::now();
    let connect_url = match Url::parse(url_str) {
        Ok(u) => u,
        Err(e) => {
            return RequestResult {
                duration: start.elapsed(),
                success: false,
                status_code: None,
                error: Some(format!("URL解析错误: {}", e)),
            };
        }
    };

    // 关键修复：将 url::Url 转换为 &str，以满足 connect_async 的 trait bound
    match connect_async(connect_url.as_str()).await {
        Ok((mut ws_stream, _)) => {
            // 连接成功
            let _connect_duration = start.elapsed();

            if let Some(msg) = message {
                // 发送消息
                if let Err(e) = ws_stream.send(Message::Text(msg.to_string())).await {
                    let total_duration = start.elapsed();
                    let error_msg = format!("WebSocket消息发送失败: {}", e);
                    let _ = ws_stream.close(None).await;
                    return RequestResult {
                        duration: total_duration,
                        success: false,
                        status_code: None, // WebSocket 没有 HTTP 状态码
                        error: Some(error_msg),
                    };
                }
            }

            if let Some(dur) = duration_secs {
                // 如果指定了持续时间，则保持连接一段时间
                tokio::time::sleep(Duration::from_secs(dur)).await;
                let total_duration = start.elapsed();
                let _ = ws_stream.close(None).await;
                RequestResult {
                    duration: total_duration,
                    success: true,
                    status_code: None, // WebSocket 没有 HTTP 状态码
                    error: None,
                }
            } else {
                // 如果没有指定持续时间，仅连接并可选地发送消息后关闭
                let _ = ws_stream.close(None).await;
                RequestResult {
                    duration: start.elapsed(),
                    success: true,
                    status_code: None, // WebSocket 没有 HTTP 状态码
                    error: None,
                }
            }
        }
        Err(e) => RequestResult {
            duration: start.elapsed(),
            success: false,
            status_code: None, // 连接失败，没有 HTTP 状态码
            error: Some(format!("WebSocket连接失败: {}", e)),
        },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let client = Client::builder()
        .timeout(Duration::from_secs(cli.timeout)) // 设置请求超时
        .build()?;

    let (tx, mut rx) = mpsc::channel(cli.concurrency * 2);

    let mut headers_map: HashMap<String, String> = HashMap::new();
    for (key, value) in &cli.headers {
        headers_map.insert(key.clone(), value.clone());
    }

    let is_websocket = cli.method.to_uppercase() == "WS";

    let actual_requests_count = if is_websocket && cli.ws_duration.is_some() {
        cli.requests // WebSocket 持续模式下，requests 是并发连接数
    } else {
        cli.requests // 其他情况，requests 是总请求数
    };

    if actual_requests_count == 0 {
        println!("错误: 总请求数 (-r) 或 WebSocket 并发数不能为 0。");
        return Ok(());
    }
    if cli.concurrency == 0 {
        println!("错误: 并发数 (-c) 不能为 0。");
        return Ok(());
    }

    println!("\n--- 压测开始 ---");
    println!("目标URL: {}", cli.url);
    println!("协议/方法: {}", if is_websocket { "WebSocket" } else { &cli.method });
    println!("并发数: {}", cli.concurrency);
    println!("请求/连接总数: {}", actual_requests_count);
    if let Some(dur) = cli.ws_duration {
        println!("WebSocket持续时间: {} 秒", dur);
    }
    if let Some(data) = &cli.data {
        println!("请求体: {}", data);
    }
    if !cli.headers.is_empty() {
        println!("自定义Header: {:?}", cli.headers);
    }

    let start_time = Instant::now();
    let mut handles = vec![];

    let requests_per_worker = actual_requests_count / cli.concurrency;
    let remainder_requests = actual_requests_count % cli.concurrency;

    for i in 0..cli.concurrency {
        let tx_clone = tx.clone();
        let client_clone = client.clone();
        let url_clone = cli.url.clone();
        let method_clone = cli.method.clone();
        let data_clone = cli.data.clone();
        let headers_clone = headers_map.clone();
        let ws_message_clone = cli.ws_message.clone();
        let ws_duration_clone = cli.ws_duration;

        let worker_requests = requests_per_worker + (if i < remainder_requests { 1 } else { 0 });

        if worker_requests == 0 {
            continue;
        }

        let handle = tokio::spawn(async move {
            if is_websocket {
                for _ in 0..worker_requests {
                    let result = make_websocket_request(
                        &url_clone,
                        ws_message_clone.as_deref(),
                        ws_duration_clone,
                    ).await;
                    if let Err(e) = tx_clone.send(result).await {
                        eprintln!("发送结果失败: {}", e);
                    }
                }
            } else {
                for _ in 0..worker_requests {
                    let result = make_http_request(
                        &client_clone,
                        &method_clone,
                        &url_clone,
                        data_clone.as_deref(),
                        &headers_clone,
                    ).await;
                    if let Err(e) = tx_clone.send(result).await {
                        eprintln!("发送结果失败: {}", e);
                    }
                }
            }
        });
        handles.push(handle);
    }

    drop(tx); // 关闭发送端，以便 rx 可以完成

    let mut histogram = Histogram::<u64>::new(3).unwrap(); // 毫秒精度
    let mut successful_requests = 0;
    let mut failed_requests = 0;
    let mut error_messages: HashMap<String, usize> = HashMap::new();
    let mut http_status_code_counts: HashMap<u16, usize> = HashMap::new(); // 用于统计 HTTP 状态码

    while let Some(result) = rx.recv().await {
        if result.success {
            successful_requests += 1;
            // 记录延迟
            if result.duration.as_millis() > 0 {
                histogram.record(result.duration.as_millis() as u64).unwrap();
            } else {
                histogram.record(1).unwrap(); // 记录为 1 毫秒，避免 HDR Histogram 报错（不能记录 0）
            }
            // 记录 HTTP 状态码
            if let Some(status) = result.status_code {
                *http_status_code_counts.entry(status.as_u16()).or_insert(0) += 1;
            }
        } else {
            failed_requests += 1;
            if let Some(err_msg) = result.error {
                *error_messages.entry(err_msg).or_insert(0) += 1;
            } else {
                *error_messages.entry("未知错误".to_string()).or_insert(0) += 1;
            }
            // 记录失败的 HTTP 请求状态码（如果存在）
            if let Some(status) = result.status_code {
                *http_status_code_counts.entry(status.as_u16()).or_insert(0) += 1;
            }
        }
    }

    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("一个并发任务执行失败: {:?}", e);
            failed_requests += 1;
        }
    }

    let total_duration = start_time.elapsed();
    let total_requests_executed = successful_requests + failed_requests;

    println!("\n--- 压测结果 ---");
    println!("总持续时间: {:.3} 秒", total_duration.as_secs_f64());
    println!("成功请求/连接数: {}", successful_requests);
    println!("失败请求/连接数: {}", failed_requests);
    println!("总请求/连接数: {}", total_requests_executed);

    if total_duration.as_secs_f64() > 0.0 {
        println!("每秒请求数 (RPS): {:.2}", total_requests_executed as f64 / total_duration.as_secs_f64());
    } else {
        println!("每秒请求数 (RPS): N/A (持续时间太短)");
    }

    if successful_requests > 0 {
        println!("平均延迟: {:.2} ms", histogram.mean());
        println!("最小延迟: {:.2} ms", histogram.min() as f64);
        println!("最大延迟: {:.2} ms", histogram.max() as f64);
        println!("延迟百分位数:");
        println!("  50% (P50): {:.2} ms", histogram.value_at_percentile(50.0) as f64);
        println!("  90% (P90): {:.2} ms", histogram.value_at_percentile(90.0) as f64);
        println!("  95% (P95): {:.2} ms", histogram.value_at_percentile(95.0) as f64);
        println!("  99% (P99): {:.2} ms", histogram.value_at_percentile(99.0) as f64);
    } else {
        println!("没有成功请求，无法计算延迟统计。");
    }

    // 打印 HTTP 状态码分布
    if !http_status_code_counts.is_empty() {
        println!("\nHTTP 状态码分布:");
        let mut sorted_status_codes: Vec<u16> = http_status_code_counts.keys().cloned().collect();
        sorted_status_codes.sort_unstable(); // 排序以便输出整洁
        for code in sorted_status_codes {
            println!("  - {}: {} 次", code, http_status_code_counts[&code]);
        }
    }

    if !error_messages.is_empty() {
        println!("\n错误详情:");
        for (msg, count) in error_messages {
            println!("  - {}: {} 次", msg, count);
        }
    }

    Ok(())
}