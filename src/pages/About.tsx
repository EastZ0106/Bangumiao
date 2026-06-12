import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function About() {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [logs, setLogs] = useState("");
  const [sending, setSending] = useState(false);
  const [msg, setMsg] = useState("");

  const handleSubmit = async () => {
    if (!description.trim()) {
      setMsg("请填写问题描述");
      return;
    }
    setSending(true);
    setMsg("");
    try {
      await invoke("send_feedback", {
        name: name.trim() || "匿名用户",
        description: description.trim(),
        logs: logs.trim(),
      });
      setMsg("已打开默认邮件客户端，请发送邮件完成反馈。感谢！");
    } catch (e) {
      setMsg("发送失败: " + String(e));
    } finally {
      setSending(false);
    }
  };

  const cardStyle: React.CSSProperties = {
    padding: "20px 24px", borderRadius: 10,
    border: "1px solid var(--border-color)", background: "var(--bg-card)",
    marginBottom: 20,
  };

  const labelStyle: React.CSSProperties = {
    fontSize: 13, fontWeight: 500, display: "block", marginBottom: 6,
  };

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "8px 12px", borderRadius: 6,
    border: "1px solid var(--border-color)", fontSize: 13,
    outline: "none", fontFamily: "inherit", marginBottom: 14,
  };

  return (
    <div style={{ maxWidth: 620 }}>
      <div className="page-header">
        <div>
          <h1 className="page-title">关于</h1>
          <p className="page-subtitle">bangumiao — 追番桌面助手</p>
        </div>
        <span className="badge" style={{ background: "#3b82f6", color: "#fff" }}>v0.2.x</span>
      </div>

      {/* Credits */}
      <div style={cardStyle}>
        <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 10 }}>致谢</h3>
        <p style={{ fontSize: 13, lineHeight: 2, color: "var(--text-secondary)" }}>
          感谢以下平台和项目对 bangumiao 的支持：<br/>
          · <b>蜜柑计划</b>（<a href="https://mikanani.me" target="_blank" rel="noreferrer">mikanani.me</a>）— 番剧 RSS 数据源<br/>
          · <b>Bangumi 番组计划</b>（<a href="https://bgm.tv" target="_blank" rel="noreferrer">bgm.tv</a>）— 二次元社区标杆<br/>
          · <b>aria2</b> — 轻量级多协议下载引擎<br/>
          · <b>Tauri</b> — 跨平台桌面应用框架
        </p>
      </div>

      {/* Author */}
      <div style={cardStyle}>
        <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 10 }}>作者</h3>
        <p style={{ fontSize: 13, lineHeight: 2, color: "var(--text-secondary)" }}>
          GitHub: <a href="https://github.com/EastZ0106" target="_blank" rel="noreferrer">EastZ0106</a><br/>
          项目仓库: <a href="https://github.com/EastZ0106/Bangumiao" target="_blank" rel="noreferrer">github.com/EastZ0106/Bangumiao</a>
        </p>
      </div>

      {/* Bug report */}
      <div style={cardStyle}>
        <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 14 }}>🐛 反馈 Bug</h3>
        <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
          请描述你遇到的问题。提交后将打开系统默认邮件客户端，发送邮件到 eastz@pku.edu.cn。
        </p>

        <label style={labelStyle}>你的称呼（选填）</label>
        <input
          type="text"
          style={inputStyle}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="方便我后续联系你"
        />

        <label style={labelStyle}>问题描述 *</label>
        <textarea
          style={{ ...inputStyle, minHeight: 100, resize: "vertical" }}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="请详细描述 Bug 的触发条件、期望行为和实际行为"
        />

        <label style={labelStyle}>终端日志 / 错误信息（选填）</label>
        <textarea
          style={{ ...inputStyle, minHeight: 80, resize: "vertical" }}
          value={logs}
          onChange={(e) => setLogs(e.target.value)}
          placeholder="粘贴 pnpm tauri dev 终端的报错信息或截图链接"
        />

        {msg && (
          <div style={{
            padding: "8px 12px", borderRadius: 6, fontSize: 12, marginBottom: 12,
            background: msg.includes("失败") ? "#fee2e2" : "#dbeafe",
            color: msg.includes("失败") ? "#991b1b" : "#1e40af",
          }}>
            {msg}
          </div>
        )}

        <button
          className="btn btn-primary"
          onClick={handleSubmit}
          disabled={sending}
          style={{ fontSize: 13, padding: "8px 20px" }}
        >
          {sending ? "处理中..." : "提交反馈"}
        </button>
      </div>
    </div>
  );
}
