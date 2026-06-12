use tauri;

#[tauri::command]
pub fn send_feedback(name: String, description: String, logs: String) -> Result<(), String> {
    let subject = "[bangumiao Bug反馈]";
    let mut body = String::new();
    body.push_str(&format!("来自: {}\n\n", name));
    body.push_str(&format!("问题描述:\n{}\n", description));
    if !logs.is_empty() {
        body.push_str(&format!("\n--- 日志/错误信息 ---\n{}\n", logs));
    }
    body.push_str("\n---\n请在此补充任何有助于定位问题的信息。\n");

    let mailto = format!(
        "mailto:eastz@pku.edu.cn?subject={}&body={}",
        urlencoding(&subject),
        urlencoding(&body),
    );

    open::that(&mailto).map_err(|e| format!("无法打开邮件客户端: {}", e))
}

fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for byte in s.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(*byte as char);
            }
            b' ' => result.push_str("%20"),
            b'\n' => result.push_str("%0A"),
            b'\r' => result.push_str("%0D"),
            b'/' => result.push_str("%2F"),
            b':' => result.push_str("%3A"),
            b'@' => result.push_str("%40"),
            b'[' => result.push_str("%5B"),
            b']' => result.push_str("%5D"),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}
