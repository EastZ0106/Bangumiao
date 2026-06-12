use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::process::{Child, Command};

pub struct Aria2Manager {
    process: Option<Child>,
    port: u16,
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
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Fast single-attempt port check — no retry loop.
    /// The retry was needed only at startup; at runtime retrying 10x
    /// just wastes ~3 s and causes thread-pool exhaustion.
    pub fn port_is_open(port: u16) -> bool {
        let addr: std::net::SocketAddr = match format!("127.0.0.1:{}", port).parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(100))
            .is_ok()
    }

    pub fn start(&mut self, aria2_path: &str, download_dir: &str, session_dir: &str) -> Result<(), String> {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "aria2c-x86_64-pc-windows-msvc.exe"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(300));

        let session_file = format!("{}/aria2.session", session_dir);
        // Ensure session directory exists (use app_dir which is already created)
        std::fs::create_dir_all(session_dir).ok();
        // Create session file if it doesn't exist (aria2c requires --input-file to exist)
        if !std::path::Path::new(&session_file).exists() {
            let _ = std::fs::write(&session_file, "");
        }

        let args = [
            "--enable-rpc",
            &format!("--rpc-listen-port={}", self.port),
            "--rpc-listen-all=false",
            "--rpc-allow-origin-all=true",
            "--follow-torrent=mem",
            "--bt-metadata-only=false",
            "--enable-dht=true",
            "--enable-dht6=false",
            "--dht-listen-port=6881-6999",
            &format!("--dir={}", download_dir),
            "--file-allocation=none",
            "--max-concurrent-downloads=5",
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
            .args(&args)
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

    fn rpc_call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "bangumiao",
            "method": method,
            "params": params,
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

        let stream_result = std::net::TcpStream::connect_timeout(
            &addr,
            std::time::Duration::from_millis(500),
        );
        let mut stream = match stream_result {
            Ok(s) => s,
            Err(_) => {
                eprintln!("[aria2] connect timeout port={}", self.port);
                return Ok(Value::Null);
            }
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(1500)))
            .ok();
        stream
            .set_write_timeout(Some(std::time::Duration::from_millis(1500)))
            .ok();
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("Write: {}", e))?;

        let mut buf = Vec::new();
        let mut temp = [0u8; 8192];
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
            None => return Ok(Value::Null),
        };

        let json_clean = json_part.trim();
        if json_clean.is_empty() {
            return Ok(Value::Null);
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
        response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No GID returned".to_string())
    }

    pub fn add_uri_with_dir(&self, uri: &str, dir: &str) -> Result<String, String> {
        let response =
            self.rpc_call("aria2.addUri", &serde_json::json!([[uri], {"dir": dir}]))?;
        response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No GID returned".to_string())
    }

    pub fn add_torrent(&self, torrent_url: &str) -> Result<String, String> {
        let torrent_data = reqwest::blocking::get(torrent_url)
            .and_then(|r| r.bytes())
            .map_err(|e| format!("Failed to fetch torrent: {}", e))?;
        let encoded = base64_encode(&torrent_data);
        let response = self.rpc_call("aria2.addTorrent", &serde_json::json!([encoded]))?;
        response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No GID returned".to_string())
    }

    pub fn tell_status(&self, gid: &str) -> Result<DownloadStatus, String> {
        let response = self.rpc_call(
            "aria2.tellStatus",
            &serde_json::json!([gid, [
                "status", "totalLength", "completedLength", "downloadSpeed", "files"
            ]]),
        )?;
        serde_json::from_value(response["result"].clone())
            .map_err(|e| format!("Failed to parse status: {}", e))
    }

    pub fn tell_active(&self) -> Result<Vec<DownloadStatus>, String> {
        eprintln!("[aria2] tell_active");
        let response = self.rpc_call(
            "aria2.tellActive",
            &serde_json::json!([[
                "gid", "status", "totalLength", "completedLength", "downloadSpeed", "files"
            ]]),
        )?;
        match response["result"].as_array() {
            Some(arr) => {
                let v: Vec<_> = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                eprintln!("[aria2] tell_active → {} tasks", v.len());
                Ok(v)
            }
            None => Ok(vec![]),
        }
    }

    pub fn tell_stopped(&self, offset: i32, num: i32) -> Result<Vec<DownloadStatus>, String> {
        eprintln!("[aria2] tell_stopped offset={} num={}", offset, num);
        let response = self.rpc_call(
            "aria2.tellStopped",
            &serde_json::json!([offset, num, [
                "gid", "status", "totalLength", "completedLength", "downloadSpeed", "files"
            ]]),
        )?;
        match response["result"].as_array() {
            Some(arr) => {
                let v: Vec<_> = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                eprintln!("[aria2] tell_stopped → {} tasks", v.len());
                Ok(v)
            }
            None => Ok(vec![]),
        }
    }

    pub fn pause(&self, gid: &str) -> Result<String, String> {
        eprintln!("[aria2] pause gid={}", gid);
        let response = self.rpc_call("aria2.pause", &serde_json::json!([gid]))?;
        let result = response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to pause".to_string())?;
        eprintln!("[aria2] pause ok → {}", result);
        Ok(result)
    }

    pub fn unpause(&self, gid: &str) -> Result<String, String> {
        eprintln!("[aria2] unpause gid={}", gid);
        let response = self.rpc_call("aria2.unpause", &serde_json::json!([gid]))?;
        let result = response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to unpause".to_string())?;
        eprintln!("[aria2] unpause ok → {}", result);
        Ok(result)
    }

    pub fn remove(&self, gid: &str) -> Result<String, String> {
        eprintln!("[aria2] remove gid={}", gid);
        let response = self.rpc_call("aria2.remove", &serde_json::json!([gid]))?;
        let result = response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to remove".to_string())?;
        eprintln!("[aria2] remove ok → {}", result);
        Ok(result)
    }

    pub fn force_remove(&self, gid: &str) -> Result<String, String> {
        eprintln!("[aria2] forceRemove gid={}", gid);
        let response = self.rpc_call("aria2.forceRemove", &serde_json::json!([gid]))?;
        let result = response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to force remove".to_string())?;
        eprintln!("[aria2] forceRemove ok → {}", result);
        Ok(result)
    }

    pub fn remove_download_result(&self, gid: &str) -> Result<String, String> {
        eprintln!("[aria2] removeDownloadResult gid={}", gid);
        let response = self.rpc_call("aria2.removeDownloadResult", &serde_json::json!([gid]))?;
        let result = response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to remove download result".to_string())?;
        eprintln!("[aria2] removeDownloadResult ok → {}", result);
        Ok(result)
    }

    pub fn add_torrent_with_dir(&self, torrent_url: &str, dir: &str) -> Result<String, String> {
        let torrent_data = reqwest::blocking::get(torrent_url)
            .and_then(|r| r.bytes())
            .map_err(|e| format!("Failed to fetch torrent: {}", e))?;
        let encoded = base64_encode(&torrent_data);
        let response =
            self.rpc_call("aria2.addTorrent", &serde_json::json!([encoded, [], {"dir": dir}]))?;
        response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No GID returned".to_string())
    }

    pub fn add_torrent_b64(&self, b64: &str) -> Result<String, String> {
        let response = self.rpc_call("aria2.addTorrent", &serde_json::json!([b64]))?;
        response["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No GID returned".to_string())
    }
}

impl Drop for Aria2Manager {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
        }
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
