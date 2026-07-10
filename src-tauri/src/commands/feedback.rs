use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

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
        utf8_percent_encode(subject, NON_ALPHANUMERIC),
        utf8_percent_encode(&body, NON_ALPHANUMERIC),
    );

    open::that(&mailto).map_err(|e| format!("无法打开邮件客户端: {}", e))
}
