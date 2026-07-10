use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const RPC_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
const RPC_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
const RPC_READ_BUFFER_SIZE: usize = 8192;

pub struct Aria2Manager {
    process: Option<Child>,
    port: u16,
    secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStatus {
    pub gid: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub total_length: String,
    #[serde(default)]
    pub completed_length: String,
    #[serde(default)]
    pub download_speed: String,
    #[serde(default)]
    pub files: Vec<Aria2File>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2File {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub length: String,
    #[serde(default)]
    pub completed_length: String,
    #[serde(default)]
    pub selected: String,
    #[serde(default)]
    pub uris: Vec<serde_json::Value>,
}

impl Aria2Manager {
    pub fn new(port: u16) -> Self {
        Aria2Manager {
            process: None,
            port,
            secret: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn new_with_secret(port: u16, secret: String) -> Self {
        Aria2Manager {
            process: None,
            port,
            secret,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn rpc_client(&self) -> Self {
        Aria2Manager::new_with_secret(self.port, self.secret.clone())
    }

    /// Fast single-attempt port check — no retry loop.
    /// The retry was needed only at startup; at runtime retrying 10x
    /// just wastes ~3 s and causes thread-pool exhaustion.
    pub fn port_is_open(port: u16) -> bool {
        let addr: std::net::SocketAddr = match format!("127.0.0.1:{}", port).parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(100)).is_ok()
    }

    pub fn start(
        &mut self,
        aria2_path: &str,
        download_dir: &str,
        session_dir: &str,
    ) -> Result<(), String> {
        self.start_with_config(aria2_path, download_dir, session_dir, 5)
    }

    pub fn start_with_config(
        &mut self,
        aria2_path: &str,
        download_dir: &str,
        session_dir: &str,
        max_concurrent_downloads: u16,
    ) -> Result<(), String> {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "aria2c-x86_64-pc-windows-msvc.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        std::thread::sleep(std::time::Duration::from_millis(300));

        let session_file = format!("{}/aria2.session", session_dir);
        // Ensure session directory exists (use app_dir which is already created)
        std::fs::create_dir_all(session_dir).ok();
        // Create session file if it doesn't exist (aria2c requires --input-file to exist)
        if !std::path::Path::new(&session_file).exists() {
            let _ = std::fs::write(&session_file, "");
        }

        self.secret = uuid::Uuid::new_v4().to_string();
        let max_concurrent = max_concurrent_downloads.clamp(1, 10);

        let args = [
            "--enable-rpc",
            &format!("--rpc-listen-port={}", self.port),
            "--rpc-listen-all=false",
            "--rpc-allow-origin-all=false",
            &format!("--rpc-secret={}", self.secret),
            "--follow-torrent=mem",
            "--bt-metadata-only=false",
            "--enable-dht=true",
            "--enable-dht6=false",
            "--dht-listen-port=6881-6999",
            &format!("--dir={}", download_dir),
            "--file-allocation=none",
            &format!("--max-concurrent-downloads={}", max_concurrent),
            "--max-connection-per-server=16",
            "--split=16",
            "--min-split-size=1M",
            "--disable-ipv6=true",
            "--check-certificate=false",
            "--quiet=true",
            "--no-conf=true",
            &format!("--save-session={}", session_file),
            &format!("--input-file={}", session_file),
            "--save-session-interval=30",
        ];

        let child = Command::new(aria2_path)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to start aria2: {}", e))?;

        self.process = Some(child);
        Ok(())
    }

    pub fn is_running(&mut self) -> bool {
        self.process
            .as_mut()
            .map(|p| p.try_wait().map(|s| s.is_none()).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn is_running_port(&self) -> bool {
        Self::port_is_open(self.port)
    }

    fn is_process_running(&self) -> bool {
        Self::port_is_open(self.port)
    }

    fn authenticated_params(&self, params: Value) -> Value {
        let mut authed = vec![Value::String(format!("token:{}", self.secret))];
        match params {
            Value::Array(items) => authed.extend(items),
            other => authed.push(other),
        }
        Value::Array(authed)
    }

    fn rpc_call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "bangumiao",
            "method": method,
            "params": self.authenticated_params(params.clone()),
        });
        let body_str = body.to_string();

        let request = format!(
            "POST /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.port,
            body_str.len(),
            body_str
        );

        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", self.port)
            .parse()
            .map_err(|e: std::net::AddrParseError| e.to_string())?;

        let stream_result = std::net::TcpStream::connect_timeout(&addr, RPC_CONNECT_TIMEOUT);
        let mut stream = match stream_result {
            Ok(s) => s,
            Err(e) => return Err(format!("Connect 127.0.0.1:{} failed: {}", self.port, e)),
        };
        stream.set_read_timeout(Some(RPC_IO_TIMEOUT)).ok();
        stream.set_write_timeout(Some(RPC_IO_TIMEOUT)).ok();
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("Write: {}", e))?;

        let mut buf = Vec::new();
        let mut temp = [0u8; RPC_READ_BUFFER_SIZE];
        loop {
            match stream.read(&mut temp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&temp[..n]);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(format!("Read error: {}", e)),
            }
        }

        let response_str = String::from_utf8_lossy(&buf);
        let json_part = match response_str.find("\r\n\r\n") {
            Some(pos) => &response_str[pos + 4..],
            None => return Err("aria2 returned an invalid HTTP response".into()),
        };

        let json_clean = json_part.trim();
        if json_clean.is_empty() {
            return Err("aria2 returned an empty response".into());
        }

        serde_json::from_str(json_clean).map_err(|e| {
            format!(
                "JSON parse: {} from '{}'",
                e,
                &json_clean[..json_clean.len().min(200)]
            )
        })
    }

    pub fn add_uri(&self, uri: &str) -> Result<String, String> {
        let response = self.rpc_call("aria2.addUri", &serde_json::json!([[uri]]))?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn add_uri_with_dir(&self, uri: &str, dir: &str) -> Result<String, String> {
        let response = self.rpc_call("aria2.addUri", &serde_json::json!([[uri], {"dir": dir}]))?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn add_torrent(&self, torrent_url: &str) -> Result<String, String> {
        let torrent_data = reqwest::blocking::get(torrent_url)
            .and_then(|r| r.bytes())
            .map_err(|e| format!("Failed to fetch torrent: {}", e))?;
        let encoded = BASE64_STANDARD.encode(&torrent_data);
        let response = self.rpc_call("aria2.addTorrent", &serde_json::json!([encoded]))?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn tell_status(&self, gid: &str) -> Result<DownloadStatus, String> {
        let response = self.rpc_call(
            "aria2.tellStatus",
            &serde_json::json!([
                gid,
                [
                    "status",
                    "totalLength",
                    "completedLength",
                    "downloadSpeed",
                    "files"
                ]
            ]),
        )?;
        serde_json::from_value(response["result"].clone())
            .map_err(|e| format!("Failed to parse status: {}", e))
    }

    pub fn tell_active(&self) -> Result<Vec<DownloadStatus>, String> {
        let response = self.rpc_call(
            "aria2.tellActive",
            &serde_json::json!([[
                "gid",
                "status",
                "totalLength",
                "completedLength",
                "downloadSpeed",
                "files"
            ]]),
        )?;
        match response["result"].as_array() {
            Some(arr) => arr
                .iter()
                .cloned()
                .map(serde_json::from_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to parse active download: {e}")),
            None => Ok(vec![]),
        }
    }

    pub fn tell_stopped(&self, offset: i32, num: i32) -> Result<Vec<DownloadStatus>, String> {
        let response = self.rpc_call(
            "aria2.tellStopped",
            &serde_json::json!([
                offset,
                num,
                [
                    "gid",
                    "status",
                    "totalLength",
                    "completedLength",
                    "downloadSpeed",
                    "files"
                ]
            ]),
        )?;
        match response["result"].as_array() {
            Some(arr) => arr
                .iter()
                .cloned()
                .map(serde_json::from_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to parse stopped download: {e}")),
            None => Ok(vec![]),
        }
    }

    pub fn pause(&self, gid: &str) -> Result<String, String> {
        self.gid_action("aria2.pause", gid, "Failed to pause")
    }

    pub fn unpause(&self, gid: &str) -> Result<String, String> {
        self.gid_action("aria2.unpause", gid, "Failed to unpause")
    }

    pub fn remove(&self, gid: &str) -> Result<String, String> {
        self.gid_action("aria2.remove", gid, "Failed to remove")
    }

    pub fn force_remove(&self, gid: &str) -> Result<String, String> {
        self.gid_action("aria2.forceRemove", gid, "Failed to force remove")
    }

    pub fn remove_download_result(&self, gid: &str) -> Result<String, String> {
        self.gid_action(
            "aria2.removeDownloadResult",
            gid,
            "Failed to remove download result",
        )
    }

    pub fn add_torrent_with_dir(&self, torrent_url: &str, dir: &str) -> Result<String, String> {
        let torrent_data = reqwest::blocking::get(torrent_url)
            .and_then(|r| r.bytes())
            .map_err(|e| format!("Failed to fetch torrent: {}", e))?;
        let encoded = BASE64_STANDARD.encode(&torrent_data);
        let response = self.rpc_call(
            "aria2.addTorrent",
            &serde_json::json!([encoded, [], {"dir": dir}]),
        )?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn add_torrent_b64(&self, b64: &str) -> Result<String, String> {
        let response = self.rpc_call("aria2.addTorrent", &serde_json::json!([b64]))?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn add_torrent_b64_with_dir(&self, b64: &str, dir: &str) -> Result<String, String> {
        let response = self.rpc_call(
            "aria2.addTorrent",
            &serde_json::json!([b64, [], {"dir": dir}]),
        )?;
        rpc_string_result(response, "No GID returned")
    }

    pub fn add_torrent_bytes_with_dir(&self, bytes: &[u8], dir: &str) -> Result<String, String> {
        self.add_torrent_b64_with_dir(&BASE64_STANDARD.encode(bytes), dir)
    }

    fn gid_action(&self, method: &str, gid: &str, error: &str) -> Result<String, String> {
        let response = self.rpc_call(method, &serde_json::json!([gid]))?;
        rpc_string_result(response, error)
    }
}

impl Drop for Aria2Manager {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
        }
    }
}

fn rpc_string_result(response: Value, fallback_error: &str) -> Result<String, String> {
    if let Some(error) = response.get("error") {
        return Err(format!("aria2 RPC error: {error}"));
    }
    response["result"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| fallback_error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_params_prefixes_rpc_token_when_secret_is_present() {
        let aria = Aria2Manager::new_with_secret(6800, "test-secret".to_string());

        let params = aria.authenticated_params(serde_json::json!([["magnet:?xt=urn:btih:test"]]));

        assert_eq!(params[0], "token:test-secret");
        assert_eq!(params[1], serde_json::json!(["magnet:?xt=urn:btih:test"]));
    }
}
