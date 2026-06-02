//! WebSocket gateway for remote clients (iOS app, web).
//!
//! Accepts WebSocket connections over TCP and bridges them to the
//! existing newline-delimited JSON protocol used by Unix socket clients.
//! This lets iOS/web clients interact with jcode sessions identically
//! to TUI clients.
//!
//! Architecture:
//!   TCP :7643  →  WebSocket upgrade  →  UnixStream::pair()  →  handle_client()
//!
//! Each WebSocket client gets a virtual UnixStream pair. One end is handed
//! to the server's existing handle_client(); the other is bridged to WebSocket
//! frames by a relay task.

use anyhow::Result;
use futures::SinkExt;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::logging;
mod auth;
mod registry;
use auth::{WsAuth, WsAuthSource, extract_ws_auth, ws_error_response};
#[cfg(test)]
pub(crate) use auth::{is_valid_hex_token, parse_bearer_token, parse_query_token};
pub use jcode_gateway_types::{PairedDevice, PairingCode};
pub use registry::DeviceRegistry;

/// Default gateway port ("jc" on phone keypad = 52, but we use 7643)
pub const DEFAULT_PORT: u16 = 7643;
const WEBSOCKET_KEEPALIVE_INTERVAL_SECS: u64 = 20;
const MAX_HTTP_BODY_BYTES: usize = 300 * 1024;

/// Gateway configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// TCP port to listen on
    pub port: u16,
    /// Bind address (default: 0.0.0.0 for Tailscale access)
    pub bind_addr: String,
    /// Whether gateway is enabled
    pub enabled: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bind_addr: "0.0.0.0".to_string(),
            enabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Gateway listener
// ---------------------------------------------------------------------------

/// Run the WebSocket gateway. Called from Server::run() as a spawned task.
///
/// For each incoming WebSocket connection:
/// 1. Extract auth token from the WebSocket upgrade request
/// 2. Validate against device registry
/// 3. Create a UnixStream::pair() - one end for the bridge, one for handle_client
/// 4. Spawn a relay task that converts WebSocket frames <-> newline-delimited JSON
/// 5. Return the server-side UnixStream for handle_client to consume
pub async fn run_gateway(
    config: GatewayConfig,
    client_tx: tokio::sync::mpsc::UnboundedSender<GatewayClient>,
) -> Result<()> {
    let addr = format!("{}:{}", config.bind_addr, config.port);
    let listener = TcpListener::bind(&addr).await?;
    logging::info(&format!("WebSocket gateway listening on {}", addr));

    let registry = Arc::new(tokio::sync::RwLock::new(DeviceRegistry::load()));

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let registry = Arc::clone(&registry);
        let client_tx = client_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(tcp_stream, peer_addr, registry, client_tx).await {
                logging::error(&format!(
                    "Gateway connection error from {}: {}",
                    peer_addr, e
                ));
            }
        });
    }
}

/// Route an incoming TCP connection: either plain HTTP (pair/health) or WebSocket.
///
/// We peek at the first chunk to check for the Upgrade: websocket header.
/// Plain HTTP requests get handled inline; WebSocket connections proceed to
/// the existing auth + bridge flow.
async fn handle_connection(
    tcp_stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    registry: Arc<tokio::sync::RwLock<DeviceRegistry>>,
    client_tx: tokio::sync::mpsc::UnboundedSender<GatewayClient>,
) -> Result<()> {
    let mut peek_buf = [0u8; 2048];
    let n = tcp_stream.peek(&mut peek_buf).await?;
    let request_head = String::from_utf8_lossy(&peek_buf[..n]);

    let is_websocket = request_head.lines().any(|line| {
        let lower = line.to_lowercase();
        lower.starts_with("upgrade:") && lower.contains("websocket")
    });

    if is_websocket {
        handle_ws_connection(tcp_stream, peer_addr, registry, client_tx).await
    } else {
        handle_http(tcp_stream, peer_addr, registry).await
    }
}

/// A gateway client ready to be plugged into handle_client
pub struct GatewayClient {
    /// The server-side end of the virtual Unix socket pair
    pub stream: crate::transport::Stream,
    /// Device info for this client
    pub device_name: String,
    /// Device ID
    pub device_id: String,
}

/// Handle a single incoming TCP connection: upgrade to WebSocket, auth, bridge.
#[expect(
    clippy::result_large_err,
    reason = "WebSocket handshake callback must return Tungstenite ErrorResponse directly"
)]
async fn handle_ws_connection(
    tcp_stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    registry: Arc<tokio::sync::RwLock<DeviceRegistry>>,
    client_tx: tokio::sync::mpsc::UnboundedSender<GatewayClient>,
) -> Result<()> {
    // Perform WebSocket handshake with a callback to inspect headers.
    // Prefer Authorization headers, but continue accepting ?token= for browser clients.
    let auth = Arc::new(std::sync::Mutex::new(None::<WsAuth>));
    let auth_cb = Arc::clone(&auth);

    let ws_stream = tokio_tungstenite::accept_hdr_async(
        tcp_stream,
        |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
         response: tokio_tungstenite::tungstenite::handshake::server::Response| {
            if request.uri().path() != "/ws" {
                return Err(ws_error_response(
                    404,
                    "Not Found",
                    "WebSocket endpoint not found",
                ));
            }

            let ws_auth = extract_ws_auth(request)?;
            let mut guard = auth_cb
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = Some(ws_auth);
            Ok(response)
        },
    )
    .await?;

    // Validate auth token
    let auth = auth
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .ok_or_else(|| anyhow::anyhow!("No auth token provided"))?;
    let token = auth.token;

    if auth.source == WsAuthSource::Query {
        logging::info(&format!(
            "Gateway: {} connected with deprecated query token auth",
            peer_addr
        ));
    }

    let (device_name, device_id) = {
        let mut reg = registry.write().await;
        // Reload from disk to pick up newly paired devices
        *reg = DeviceRegistry::load();
        match reg.validate_token(&token) {
            Some(device) => {
                let name = device.name.clone();
                let id = device.id.clone();
                reg.touch_device(&token);
                (name, id)
            }
            None => {
                anyhow::bail!("Invalid auth token from {}", peer_addr);
            }
        }
    };

    logging::info(&format!(
        "Gateway: {} connected (device: {}, addr: {})",
        device_name, device_id, peer_addr
    ));

    // Create a virtual Unix socket pair
    let (server_stream, bridge_stream) = crate::transport::stream_pair()
        .map_err(|e| anyhow::anyhow!("Failed to create socket pair: {}", e))?;

    // Send the server-side stream to the main server loop for handle_client
    client_tx.send(GatewayClient {
        stream: server_stream,
        device_name: device_name.clone(),
        device_id,
    })?;

    // Bridge WebSocket frames <-> newline-delimited JSON on the bridge stream
    let (ws_sink, ws_source) = ws_stream.split();
    let ws_sink = Arc::new(tokio::sync::Mutex::new(ws_sink));

    let (bridge_reader, bridge_writer) = bridge_stream.into_split();
    let mut bridge_reader = BufReader::new(bridge_reader);
    let bridge_writer = Arc::new(tokio::sync::Mutex::new(bridge_writer));

    // Task 1: WebSocket → Unix socket (client requests)
    let writer_for_ws = Arc::clone(&bridge_writer);
    let sink_for_ping = Arc::clone(&ws_sink);
    let sink_for_unix = Arc::clone(&ws_sink);
    let sink_for_keepalive = Arc::clone(&ws_sink);
    let ws_to_unix = tokio::spawn(async move {
        let mut ws_source = ws_source;
        while let Some(msg) = ws_source.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let mut writer = writer_for_ws.lock().await;
                    if text.ends_with('\n') {
                        if writer.write_all(text.as_bytes()).await.is_err() {
                            break;
                        }
                    } else {
                        if writer.write_all(text.as_bytes()).await.is_err() {
                            break;
                        }
                        if writer.write_all(b"\n").await.is_err() {
                            break;
                        }
                    }
                    if writer.flush().await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    let mut sink = sink_for_ping.lock().await;
                    let _ = sink.send(Message::Pong(data)).await;
                }
                Err(_) => break,
                _ => {}
            }
        }
    });

    let keepalive_device_name = device_name.clone();
    let keepalive = tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(WEBSOCKET_KEEPALIVE_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let mut sink = sink_for_keepalive.lock().await;
            if sink.send(Message::Ping(Vec::new())).await.is_err() {
                logging::info(&format!(
                    "Gateway: stopping keepalive for {} after ping send failure",
                    keepalive_device_name
                ));
                break;
            }
        }
    });

    // Task 2: Unix socket → WebSocket (server events)
    let unix_to_ws = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            match bridge_reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim_end().to_string();
                    if !trimmed.is_empty() {
                        let mut sink = sink_for_unix.lock().await;
                        if sink.send(Message::Text(trimmed)).await.is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Wait for either direction to finish
    tokio::pin!(ws_to_unix);
    tokio::pin!(unix_to_ws);
    tokio::pin!(keepalive);

    tokio::select! {
        _ = &mut ws_to_unix => {}
        _ = &mut unix_to_ws => {}
        _ = &mut keepalive => {}
    }

    ws_to_unix.abort();
    unix_to_ws.abort();
    keepalive.abort();

    logging::info(&format!("Gateway: {} disconnected", device_name));
    Ok(())
}

fn http_response(status: u16, status_text: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n{}",
        status,
        status_text,
        body.as_bytes().len(),
        body
    ).into_bytes()
}

async fn read_http_request(tcp_stream: &mut tokio::net::TcpStream) -> Result<String> {
    let mut request_bytes = vec![0u8; 8192];
    let mut bytes_read = tcp_stream.read(&mut request_bytes).await?;
    request_bytes.truncate(bytes_read);

    loop {
        let header_end = request_bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4);
        let Some(header_end) = header_end else {
            let mut chunk = [0u8; 8192];
            let n = tcp_stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            request_bytes.extend_from_slice(&chunk[..n]);
            bytes_read += n;
            if bytes_read > MAX_HTTP_BODY_BYTES + 8192 {
                anyhow::bail!("HTTP request is too large");
            }
            continue;
        };

        let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
        let content_length = content_length_from_headers(&headers).unwrap_or(0);
        if content_length > MAX_HTTP_BODY_BYTES {
            anyhow::bail!("HTTP body is too large");
        }
        let body_read = request_bytes.len().saturating_sub(header_end);
        if body_read >= content_length {
            break;
        }
        let mut chunk = vec![0u8; (content_length - body_read).min(8192)];
        let n = tcp_stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        request_bytes.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&request_bytes).into_owned())
}

fn content_length_from_headers(headers: &str) -> Option<usize> {
    header_value(headers, "content-length")?.parse().ok()
}

/// Handle a plain HTTP request (not WebSocket).
/// Supports:
///   GET  /health            - server status
///   POST /pair              - exchange pairing code for auth token
///   GET  /knowledge/files   - list editable identity/wiki files
///   GET  /knowledge/file    - read one editable identity/wiki file
///   POST /knowledge/file    - update one editable identity/wiki file
///   OPTIONS *               - CORS preflight
async fn handle_http(
    mut tcp_stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    registry: Arc<tokio::sync::RwLock<DeviceRegistry>>,
) -> Result<()> {
    let request = read_http_request(&mut tcp_stream).await?;

    let first_line = request.lines().next().unwrap_or("");
    let (method, path) = {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            (parts[0], parts[1])
        } else {
            ("", "")
        }
    };

    // Strip query params from path for matching
    let path_base = path.split('?').next().unwrap_or(path);

    logging::info(&format!(
        "Gateway HTTP: {} {} from {}",
        method, path_base, peer_addr
    ));

    let response = match (method, path_base) {
        ("GET", "/health") => {
            let body = serde_json::json!({
                "status": "ok",
                "version": env!("JCODE_VERSION"),
                "gateway": true,
            });
            http_response(200, "OK", &body.to_string())
        }

        ("POST", "/pair") => {
            let body_str = http_body(&request);
            handle_pair_request(body_str, &registry).await
        }

        ("GET", "/knowledge/files") => {
            if let Err(response) = authorize_http_request(&request, &registry).await {
                response
            } else {
                let query = path.split_once('?').map(|(_, query)| query).unwrap_or("");
                handle_knowledge_list_request(query).await
            }
        }

        ("GET", "/knowledge/file") => {
            if let Err(response) = authorize_http_request(&request, &registry).await {
                response
            } else {
                let query = path.split_once('?').map(|(_, query)| query).unwrap_or("");
                handle_knowledge_read_request(query).await
            }
        }

        ("POST", "/knowledge/file") => {
            if let Err(response) = authorize_http_request(&request, &registry).await {
                response
            } else {
                handle_knowledge_write_request(http_body(&request)).await
            }
        }

        ("OPTIONS", _) => {
            // CORS preflight
            "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nAccess-Control-Max-Age: 86400\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string().into_bytes()
        }

        _ => {
            let body = serde_json::json!({"error": "Not found"});
            http_response(404, "Not Found", &body.to_string())
        }
    };

    tcp_stream.write_all(&response).await?;
    tcp_stream.shutdown().await?;
    Ok(())
}

fn http_body(request: &str) -> &str {
    request.split("\r\n\r\n").nth(1).unwrap_or("")
}

fn header_value<'a>(request: &'a str, header_name: &str) -> Option<&'a str> {
    request.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim())
        } else {
            None
        }
    })
}

async fn authorize_http_request(
    request: &str,
    registry: &Arc<tokio::sync::RwLock<DeviceRegistry>>,
) -> std::result::Result<(), Vec<u8>> {
    let Some(header) = header_value(request, "authorization") else {
        let body = serde_json::json!({"error": "Missing Authorization bearer token"});
        return Err(http_response(401, "Unauthorized", &body.to_string()));
    };
    let Some(token) = auth::parse_bearer_token(header) else {
        let body = serde_json::json!({"error": "Invalid Authorization bearer token"});
        return Err(http_response(401, "Unauthorized", &body.to_string()));
    };

    let mut reg = registry.write().await;
    *reg = DeviceRegistry::load();
    if reg.validate_token(token).is_some() {
        reg.touch_device(token);
        Ok(())
    } else {
        let body = serde_json::json!({"error": "Invalid or expired auth token"});
        Err(http_response(401, "Unauthorized", &body.to_string()))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum KnowledgeScope {
    Identity,
    Wiki,
}

#[derive(Debug, Serialize)]
struct KnowledgeFileSummary {
    scope: KnowledgeScope,
    path: String,
    title: String,
    byte_len: u64,
    modified_unix_secs: Option<u64>,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct KnowledgeFileRead {
    scope: KnowledgeScope,
    path: String,
    title: String,
    content: String,
    sha256: String,
    byte_len: u64,
    modified_unix_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeWriteRequest {
    scope: KnowledgeScope,
    path: String,
    content: String,
    base_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
struct KnowledgeWriteResponse {
    scope: KnowledgeScope,
    path: String,
    sha256: String,
    backup_path: Option<String>,
}

async fn handle_knowledge_list_request(query: &str) -> Vec<u8> {
    match knowledge_scope_from_query(query).and_then(list_knowledge_files) {
        Ok(files) => http_response(
            200,
            "OK",
            &serde_json::json!({ "files": files }).to_string(),
        ),
        Err(error) => json_error_response(400, "Bad Request", &error.to_string()),
    }
}

async fn handle_knowledge_read_request(query: &str) -> Vec<u8> {
    let result = knowledge_scope_from_query(query).and_then(|scope| {
        let path = query_param(query, "path")
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing path"))?;
        read_knowledge_file(scope, &path)
    });

    match result {
        Ok(file) => http_response(200, "OK", &serde_json::to_string(&file).unwrap()),
        Err(error) => json_error_response(400, "Bad Request", &error.to_string()),
    }
}

async fn handle_knowledge_write_request(body: &str) -> Vec<u8> {
    let req: KnowledgeWriteRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(error) => {
            return json_error_response(400, "Bad Request", &format!("Invalid JSON: {error}"));
        }
    };

    match write_knowledge_file(req) {
        Ok(response) => http_response(200, "OK", &serde_json::to_string(&response).unwrap()),
        Err(error) => json_error_response(400, "Bad Request", &error.to_string()),
    }
}

fn json_error_response(status: u16, status_text: &str, message: &str) -> Vec<u8> {
    let body = serde_json::json!({ "error": message });
    http_response(status, status_text, &body.to_string())
}

fn knowledge_scope_from_query(query: &str) -> Result<KnowledgeScope> {
    match query_param(query, "scope")
        .unwrap_or_else(|| "identity".to_string())
        .as_str()
    {
        "identity" => Ok(KnowledgeScope::Identity),
        "wiki" => Ok(KnowledgeScope::Wiki),
        other => anyhow::bail!("unsupported knowledge scope '{other}'"),
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(name) == key {
            Some(percent_decode(value))
        } else {
            None
        }
    })
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
                    && let Ok(value) = u8::from_str_radix(hex, 16)
                {
                    output.push(value);
                    index += 3;
                    continue;
                }
                output.push(bytes[index]);
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn list_knowledge_files(scope: KnowledgeScope) -> Result<Vec<KnowledgeFileSummary>> {
    let root = knowledge_root(scope)?;
    let mut files = Vec::new();
    for relative_path in allowed_knowledge_paths(scope, &root)? {
        let absolute_path = root.join(&relative_path);
        if !absolute_path.is_file() {
            continue;
        }
        files.push(file_summary(scope, &root, &relative_path)?);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn read_knowledge_file(scope: KnowledgeScope, path: &str) -> Result<KnowledgeFileRead> {
    let root = knowledge_root(scope)?;
    let relative_path = validate_knowledge_path(scope, &root, path)?;
    let absolute_path = root.join(&relative_path);
    let content = std::fs::read_to_string(&absolute_path)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", relative_path.display()))?;
    let metadata = std::fs::metadata(&absolute_path)?;
    Ok(KnowledgeFileRead {
        scope,
        path: slash_path(&relative_path),
        title: knowledge_title(&relative_path, &content),
        sha256: sha256_hex(content.as_bytes()),
        byte_len: metadata.len(),
        modified_unix_secs: metadata_modified_unix_secs(&metadata),
        content,
    })
}

fn write_knowledge_file(req: KnowledgeWriteRequest) -> Result<KnowledgeWriteResponse> {
    if req.content.len() > 256 * 1024 {
        anyhow::bail!("file is too large for mobile editing");
    }
    reject_secret_like_content(&req.content)?;

    let root = knowledge_root(req.scope)?;
    let relative_path = validate_knowledge_path(req.scope, &root, &req.path)?;
    let absolute_path = root.join(&relative_path);
    let existing = std::fs::read_to_string(&absolute_path)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", relative_path.display()))?;
    let existing_hash = sha256_hex(existing.as_bytes());
    if let Some(base_hash) = req.base_sha256.as_deref()
        && base_hash != existing_hash
    {
        anyhow::bail!("file changed on disk; reload before saving");
    }

    let backup_path = backup_knowledge_file(req.scope, &absolute_path, &relative_path)?;
    std::fs::write(&absolute_path, req.content.as_bytes())
        .map_err(|error| anyhow::anyhow!("failed to write {}: {error}", relative_path.display()))?;
    Ok(KnowledgeWriteResponse {
        scope: req.scope,
        path: slash_path(&relative_path),
        sha256: sha256_hex(req.content.as_bytes()),
        backup_path: backup_path.map(|path| path.display().to_string()),
    })
}

fn knowledge_root(scope: KnowledgeScope) -> Result<PathBuf> {
    match scope {
        KnowledgeScope::Identity => std::env::current_dir()
            .map_err(|error| anyhow::anyhow!("failed to resolve current workspace: {error}")),
        KnowledgeScope::Wiki => Ok(home_dir()?.join("brain/wiki")),
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))
}

fn allowed_knowledge_paths(scope: KnowledgeScope, root: &Path) -> Result<Vec<PathBuf>> {
    match scope {
        KnowledgeScope::Identity => Ok(identity_paths(root)),
        KnowledgeScope::Wiki => wiki_paths(root),
    }
}

fn identity_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for path in ["AGENTS.md", "CLAUDE.md"] {
        if root.join(path).is_file() {
            paths.push(PathBuf::from(path));
        }
    }
    collect_matching_files(&root.join(".claude/agents"), root, &mut paths, |path| {
        path.extension().is_some_and(|ext| ext == "md")
    });
    collect_matching_files(&root.join(".claude/skills"), root, &mut paths, |path| {
        path.file_name().is_some_and(|name| name == "SKILL.md")
    });
    let settings = PathBuf::from(".claude/settings.json");
    if root.join(&settings).is_file() {
        paths.push(settings);
    }
    paths
}

fn wiki_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for path in ["index.md", "AGENTS.md", "log.md"] {
        if root.join(path).is_file() {
            paths.push(PathBuf::from(path));
        }
    }
    for dir in ["concepts", "entities", "answers", "sources"] {
        collect_matching_files(&root.join(dir), root, &mut paths, |path| {
            path.extension().is_some_and(|ext| ext == "md")
        });
    }
    Ok(paths)
}

fn collect_matching_files(
    dir: &Path,
    root: &Path,
    paths: &mut Vec<PathBuf>,
    matches: fn(&Path) -> bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_matching_files(&path, root, paths, matches);
        } else if matches(&path)
            && let Ok(relative) = path.strip_prefix(root)
        {
            paths.push(relative.to_path_buf());
        }
    }
}

fn validate_knowledge_path(scope: KnowledgeScope, root: &Path, path: &str) -> Result<PathBuf> {
    let relative_path = safe_relative_path(path)?;
    let allowed = allowed_knowledge_paths(scope, root)?;
    if allowed.iter().any(|allowed| allowed == &relative_path) {
        Ok(relative_path)
    } else {
        anyhow::bail!("path is not editable from mobile")
    }
}

fn safe_relative_path(path: &str) -> Result<PathBuf> {
    let path = Path::new(path.trim_start_matches('/'));
    if path.as_os_str().is_empty() {
        anyhow::bail!("path cannot be empty");
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        anyhow::bail!("path must stay inside the editable knowledge root");
    }
    Ok(path.to_path_buf())
}

fn file_summary(
    scope: KnowledgeScope,
    root: &Path,
    relative_path: &Path,
) -> Result<KnowledgeFileSummary> {
    let absolute_path = root.join(relative_path);
    let content = std::fs::read_to_string(&absolute_path).unwrap_or_default();
    let metadata = std::fs::metadata(&absolute_path)?;
    Ok(KnowledgeFileSummary {
        scope,
        path: slash_path(relative_path),
        title: knowledge_title(relative_path, &content),
        byte_len: metadata.len(),
        modified_unix_secs: metadata_modified_unix_secs(&metadata),
        sha256: sha256_hex(content.as_bytes()),
    })
}

fn knowledge_title(relative_path: &Path, content: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            relative_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| slash_path(relative_path))
}

fn metadata_modified_unix_secs(metadata: &std::fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn reject_secret_like_content(content: &str) -> Result<()> {
    let lowered = content.to_ascii_lowercase();
    for marker in [
        "ghp_",
        "sk-",
        "api_key=",
        "api-key",
        "access_token",
        "refresh_token",
        "private key",
        "-----begin",
    ] {
        if lowered.contains(marker) {
            anyhow::bail!("mobile knowledge edits cannot save secret-like content");
        }
    }
    Ok(())
}

fn backup_knowledge_file(
    scope: KnowledgeScope,
    absolute_path: &Path,
    relative_path: &Path,
) -> Result<Option<PathBuf>> {
    if !absolute_path.exists() {
        return Ok(None);
    }
    let scope_name = match scope {
        KnowledgeScope::Identity => "identity",
        KnowledgeScope::Wiki => "wiki",
    };
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup_path = home_dir()?
        .join(".jcode/mobile-edit-backups")
        .join(scope_name)
        .join(format!(
            "{timestamp}-{}",
            slash_path(relative_path).replace('/', "__")
        ));
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(absolute_path, &backup_path)?;
    Ok(Some(backup_path))
}

/// Handle POST /pair request.
///
/// Expected JSON body:
/// ```json
/// {
///   "code": "123456",
///   "device_id": "uuid-here",
///   "device_name": "Jeremy's iPhone",
///   "apns_token": "optional-apns-token"
/// }
/// ```
///
/// Returns:
/// ```json
/// {
///   "token": "hex-auth-token",
///   "server_name": "jcode",
///   "server_version": "v0.4.0"
/// }
/// ```
async fn handle_pair_request(
    body: &str,
    registry: &Arc<tokio::sync::RwLock<DeviceRegistry>>,
) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct PairRequest {
        code: String,
        device_id: String,
        device_name: String,
        apns_token: Option<String>,
    }

    let req: PairRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let body = serde_json::json!({"error": format!("Invalid JSON: {}", e)});
            return http_response(400, "Bad Request", &body.to_string());
        }
    };

    let mut reg = registry.write().await;

    // Reload from disk - pairing codes are generated by `jcode pair` CLI
    *reg = DeviceRegistry::load();

    if !reg.validate_code(&req.code) {
        let body = serde_json::json!({"error": "Invalid or expired pairing code"});
        return http_response(401, "Unauthorized", &body.to_string());
    }

    let token = reg.pair_device(
        req.device_id.clone(),
        req.device_name.clone(),
        req.apns_token,
    );

    logging::info(&format!(
        "Gateway: paired device '{}' ({})",
        req.device_name, req.device_id
    ));

    let body = serde_json::json!({
        "token": token,
        "server_name": "jcode",
        "server_version": env!("JCODE_VERSION"),
    });
    http_response(200, "OK", &body.to_string())
}

#[cfg(test)]
#[path = "gateway_tests.rs"]
mod gateway_tests;
