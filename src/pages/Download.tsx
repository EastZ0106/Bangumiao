import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DownloadItem {
  id: string;
  episode_title: string;
  status: string;
  progress: number;
  file_path: string;
  subscription_title?: string;
}

export default function Download() {
  const [items, setItems] = useState<DownloadItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [torrentUrl, setTorrentUrl] = useState("");
  const [torrentTitle, setTorrentTitle] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loadDownloads = useCallback(async () => {
    try {
      const data = await invoke<DownloadItem[]>("get_downloads");
      setItems(data);
      setLoading(false);
    } catch (e) {
      console.error("Failed to load downloads:", e);
      setLoading(false);
    }
  }, []);

  const pollProgress = useCallback(async () => {
    try {
      const log = await invoke<string>("sync_downloads");
      if (log && log.length > 5) setErrorMsg(log);
      const data = await invoke<DownloadItem[]>("get_downloads");
      setItems(data);
    } catch (e) { console.error(e); }
  }, []);

  // Start/stop polling based on visibility
  useEffect(() => {
    loadDownloads();

    const startPolling = () => {
      if (pollingRef.current) return;
      pollProgress(); // immediate first sync
      pollingRef.current = setInterval(pollProgress, 2000);
    };

    const stopPolling = () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
        pollingRef.current = null;
      }
    };

    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        startPolling();
      } else {
        stopPolling();
      }
    };

    startPolling();
    document.addEventListener("visibilitychange", handleVisibility);

    return () => {
      stopPolling();
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [loadDownloads, pollProgress]);

  const handlePause = async (id: string) => {
    try { await invoke("pause_download", { id }); loadDownloads(); } catch (e) { console.error(e); }
  };

  const handleResume = async (id: string) => {
    try { await invoke("resume_download", { id }); loadDownloads(); } catch (e) { console.error(e); }
  };

  const handleRemove = async (id: string) => {
    try { await invoke("remove_download", { id }); loadDownloads(); } catch (e) { console.error(e); }
  };

  const [errorMsg, setErrorMsg] = useState("");

  const handleCleanDir = async () => {
    setErrorMsg("正在清理...");
    try {
      const msg = await invoke<string>("clean_download_dir");
      setErrorMsg(msg);
      setTimeout(() => setErrorMsg(""), 3000);
    } catch (e) {
      setErrorMsg("清理失败: " + String(e));
    }
  };

  const handleAddTorrent = async () => {
    if (!torrentUrl.trim() || !torrentTitle.trim()) return;
    setSubmitting(true);
    setErrorMsg("");
    try {
      setErrorMsg("正在添加下载任务...");
      await invoke("add_torrent_download", { torrentUrl: torrentUrl.trim(), title: torrentTitle.trim() });
      setErrorMsg("下载任务已添加！");
      setTorrentUrl("");
      setTorrentTitle("");
      setShowForm(false);
      loadDownloads();
      setTimeout(() => setErrorMsg(""), 2000);
    } catch (e) {
      setErrorMsg("添加失败: " + String(e));
      console.error("Failed to add torrent:", e);
    } finally {
      setSubmitting(false);
    }
  };

  const statusLabel = (s: string) => {
    switch (s) {
      case "pending": return <span className="badge badge-warning">等待中</span>;
      case "active": return <span className="badge badge-info">下载中</span>;
      case "downloading": return <span className="badge badge-info">下载中</span>;
      case "paused": return <span className="badge badge-warning">已暂停</span>;
      case "completed": return <span className="badge badge-success">已完成</span>;
      case "failed": return <span className="badge badge-error">失败</span>;
      default: return <span className="badge">{s}</span>;
    }
  };

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">下载管理</h1>
          <p className="page-subtitle">查看和管理正在下载与已完成的番剧</p>
        </div>
        <button className="btn btn-outline" onClick={handleCleanDir} style={{ marginRight: 8 }}>
          清理残留文件
        </button>
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? "取消" : "添加下载"}
        </button>
      </div>

      {showForm && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <input
              type="text"
              placeholder="磁链 (magnet:) 或 .torrent 文件路径（如 C:\Users\WINDOWS\Downloads\xxx.torrent）"
              value={torrentUrl}
              onChange={(e) => setTorrentUrl(e.target.value)}
              style={{
                padding: "8px 12px", borderRadius: 6, border: "1px solid var(--border-color)",
                fontSize: 13, outline: "none", fontFamily: "inherit",
              }}
            />
            <input
              type="text"
              placeholder="下载标题（如：番剧名 - 第01话）"
              value={torrentTitle}
              onChange={(e) => setTorrentTitle(e.target.value)}
              style={{
                padding: "8px 12px", borderRadius: 6, border: "1px solid var(--border-color)",
                fontSize: 13, outline: "none", fontFamily: "inherit",
              }}
            />
            <button
              className="btn btn-primary"
              onClick={handleAddTorrent}
              disabled={submitting || !torrentUrl.trim() || !torrentTitle.trim()}
              style={{ alignSelf: "flex-start" }}
            >
              {submitting ? "添加中..." : "开始下载"}
            </button>
            {errorMsg && (
              <div style={{ fontSize: 12, padding: "6px 10px", borderRadius: 6, background: errorMsg.includes("失败") ? "#fee2e2" : "#dbeafe", color: errorMsg.includes("失败") ? "#991b1b" : "#1e40af" }}>
                {errorMsg}
              </div>
            )}
          </div>
        </div>
      )}

      {loading ? (
        <div className="empty-state"><p>加载中...</p></div>
      ) : items.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">⬇</div>
          <div className="empty-state-title">暂无下载任务</div>
          <p>点击上方「添加下载」输入种子链接开始下载</p>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {items.map((d) => (
            <div className="card" key={d.id} style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 500, marginBottom: 4 }}>{d.episode_title}</div>
                {d.subscription_title && (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {d.subscription_title}
                  </div>
                )}
              </div>
              <div style={{ minWidth: 150 }}>
                {(d.status === "downloading" || d.status === "active") ? (
                  <div>
                    <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, marginBottom: 4 }}>
                      <span>{(d.progress * 100).toFixed(1)}%</span>
                    </div>
                    <div className="progress-bar">
                      <div className="progress-bar-fill" style={{ width: `${d.progress * 100}%` }} />
                    </div>
                  </div>
                ) : (
                  statusLabel(d.status)
                )}
              </div>
              <div style={{ display: "flex", gap: 4 }}>
                {(d.status === "downloading" || d.status === "active") && (
                  <button className="btn btn-ghost" style={{ fontSize: 12, padding: "3px 8px" }} onClick={() => handlePause(d.id)}>暂停</button>
                )}
                {d.status === "paused" && (
                  <button className="btn btn-ghost" style={{ fontSize: 12, padding: "3px 8px" }} onClick={() => handleResume(d.id)}>恢复</button>
                )}
                <button className="btn btn-ghost" style={{ fontSize: 12, padding: "3px 8px", color: "var(--color-error)" }} onClick={() => handleRemove(d.id)}>删除</button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
