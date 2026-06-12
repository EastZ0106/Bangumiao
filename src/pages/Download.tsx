import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DownloadItem {
  id: string;
  episode_title: string;
  status: string;
  progress: number;
  file_path: string;
  subscription_title?: string;
  subscription_id?: string;
  episode_number?: number;
  gid?: string;
}

interface PendingGroup {
  subscription_title: string;
  subscription_id: string;
  episodes: DownloadItem[];
}

export default function Download() {
  const [items, setItems] = useState<DownloadItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [torrentUrl, setTorrentUrl] = useState("");
  const [torrentTitle, setTorrentTitle] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Pending state
  const [pendingGroups, setPendingGroups] = useState<PendingGroup[]>([]);
  const [showPending, setShowPending] = useState(true);
  const [selectedEpisodes, setSelectedEpisodes] = useState<Set<string>>(new Set());
  const [batchStarting, setBatchStarting] = useState(false);

  const [toast, setToast] = useState<{ text: string; ok: boolean } | null>(null);
  const showToast = (text: string, ok: boolean) => {
    setToast({ text, ok });
    setTimeout(() => setToast(null), 4000);
  };

  const loadDownloads = useCallback(async () => {
    try {
      const data = await invoke<DownloadItem[]>("get_downloads");
      setItems(data.filter(d => d.status !== "pending"));
      setLoading(false);
    } catch (e) {
      console.error("Failed to load downloads:", e);
      setLoading(false);
    }
  }, []);

  const loadPending = useCallback(async () => {
    try {
      const groups = await invoke<PendingGroup[]>("get_pending_episodes");
      setPendingGroups(groups);
    } catch (e) {
      console.error("Failed to load pending:", e);
    }
  }, []);

  const pollProgress = useCallback(async () => {
    try {
      await invoke<string>("sync_downloads");
      const data = await invoke<DownloadItem[]>("get_downloads");
      setItems(data);
    } catch (e) { console.error(e); }
  }, []);

  useEffect(() => {
    loadDownloads();
    loadPending();

    const startPolling = () => {
      if (pollingRef.current) return;
      pollProgress();
      pollingRef.current = setInterval(pollProgress, 2000);
    };

    const stopPolling = () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
        pollingRef.current = null;
      }
    };

    const handleVisibility = () => {
      if (document.visibilityState === "visible") startPolling();
      else stopPolling();
    };

    startPolling();
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      stopPolling();
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [loadDownloads, pollProgress, loadPending]);

  const handlePause = async (id: string) => {
    try { await invoke("pause_download", { id }); loadDownloads(); } catch (e) { console.error(e); }
  };

  const handleResume = async (id: string) => {
    try { await invoke("resume_download", { id }); loadDownloads(); } catch (e) { console.error(e); }
  };

  const handleRemove = async (id: string) => {
    try { await invoke("remove_download", { id }); loadDownloads(); loadPending(); } catch (e) { console.error(e); }
  };

  const handleCleanDir = async () => {
    if (!confirm("确定要清理所有 .torrent 和 .aria2 残留文件吗？\n\n此操作会递归清理下载目录及所有子目录中的中间文件，不会删除已下载的视频。")) return;
    setCleaning(true);
    try {
      const msg = await invoke<string>("clean_download_dir");
      showToast(msg, true);
    } catch (e) {
      showToast("清理失败: " + String(e), false);
    } finally {
      setCleaning(false);
    }
  };

  const handleAddTorrent = async () => {
    if (!torrentUrl.trim() || !torrentTitle.trim()) return;
    setSubmitting(true);
    try {
      await invoke("add_torrent_download", { torrentUrl: torrentUrl.trim(), title: torrentTitle.trim() });
      setTorrentUrl("");
      setTorrentTitle("");
      setShowForm(false);
      loadDownloads();
      showToast("下载任务已添加", true);
    } catch (e) {
      showToast("添加失败: " + String(e), false);
    } finally {
      setSubmitting(false);
    }
  };

  const toggleSelect = (id: string) => {
    setSelectedEpisodes(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const allIds = pendingGroups.flatMap(g => g.episodes.map(e => e.id));
    if (allIds.length > 0 && allIds.every(id => selectedEpisodes.has(id))) {
      setSelectedEpisodes(new Set());
    } else {
      setSelectedEpisodes(new Set(allIds));
    }
  };

  const handleStartSelected = async () => {
    if (selectedEpisodes.size === 0) return;
    setBatchStarting(true);
    try {
      const ids = Array.from(selectedEpisodes);
      await invoke("batch_start_downloads", { episodeIds: ids });
      showToast(`已开始 ${ids.length} 个下载任务`, true);
      setSelectedEpisodes(new Set());
      loadPending();
      loadDownloads();
    } catch (e) {
      showToast("启动下载失败: " + String(e), false);
    } finally {
      setBatchStarting(false);
    }
  };

  const handleStartSingle = async (id: string) => {
    try {
      await invoke("start_download", { episodeId: id });
      showToast("下载已开始", true);
      setSelectedEpisodes(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
      loadPending();
      loadDownloads();
    } catch (e) {
      showToast("启动下载失败: " + String(e), false);
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
      {/* ── Toast ── */}
      {toast && (
        <div style={{
          position: "fixed", top: 16, right: 16, zIndex: 2000,
          padding: "10px 16px", borderRadius: 8, fontSize: 13,
          background: toast.ok ? "#dbeafe" : "#fee2e2",
          color: toast.ok ? "#1e40af" : "#991b1b",
          boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
          maxWidth: 400,
        }}>
          {toast.ok ? "✓ " : "✗ "}{toast.text}
        </div>
      )}

      <div className="page-header">
        <div>
          <h1 className="page-title">下载管理</h1>
          <p className="page-subtitle">查看和管理正在下载与已完成的番剧</p>
        </div>
        <button
          className="btn btn-outline"
          onClick={handleCleanDir}
          disabled={cleaning}
          style={{ marginRight: 8 }}
        >
          {cleaning ? "清理中..." : "清理残留文件"}
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
              placeholder="磁链 (magnet:) 或 .torrent 文件路径"
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
          </div>
        </div>
      )}

      {/* ── Pending episodes section ── */}
      {pendingGroups.length > 0 && (
        <>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12, marginTop: 8 }}>
            <button
              className="btn btn-ghost"
              onClick={() => setShowPending(!showPending)}
              style={{ fontSize: 13, fontWeight: 600 }}
            >
              {showPending ? "▾" : "▸"} 待处理 ({pendingGroups.flatMap(g => g.episodes).length})
            </button>
            {showPending && (
              <>
                <button className="btn btn-ghost" style={{ fontSize: 11 }} onClick={toggleSelectAll}>
                  {(() => {
                    const allIds = pendingGroups.flatMap(g => g.episodes.map(e => e.id));
                    return allIds.length > 0 && allIds.every(id => selectedEpisodes.has(id)) ? "取消全选" : "全选";
                  })()}
                </button>
                <button
                  className="btn btn-primary"
                  disabled={selectedEpisodes.size === 0 || batchStarting}
                  onClick={handleStartSelected}
                  style={{ fontSize: 12, padding: "4px 14px" }}
                >
                  {batchStarting ? "启动中..." : `下载选中 (${selectedEpisodes.size})`}
                </button>
              </>
            )}
          </div>

          {showPending && (
            <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 16 }}>
              {pendingGroups.map((group) => (
                <div key={group.subscription_id} style={{
                  borderRadius: 10, border: "1px solid var(--border-color)",
                  background: "var(--bg-card)", padding: "12px 16px",
                }}>
                  <div style={{
                    fontWeight: 600, fontSize: 14, marginBottom: 8,
                    paddingBottom: 6, borderBottom: "1px solid var(--border-color)",
                    display: "flex", justifyContent: "space-between", alignItems: "center",
                  }}>
                    <span>{group.subscription_title}</span>
                    <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                      {group.episodes.length} 集
                    </span>
                  </div>
                  {group.episodes.map((ep) => {
                    const dupNumber = group.episodes.filter(e => e.episode_number != null && e.episode_number === ep.episode_number).length > 1;
                    return (
                      <div key={ep.id} style={{
                        display: "flex", alignItems: "center", gap: 8,
                        padding: "6px 0", fontSize: 13,
                        borderBottom: "1px solid var(--border-color)",
                        background: dupNumber ? "#fef3c7" : "transparent",
                        borderRadius: 4,
                      }}>
                        <input
                          type="checkbox"
                          checked={selectedEpisodes.has(ep.id)}
                          onChange={() => toggleSelect(ep.id)}
                          style={{ cursor: "pointer", flexShrink: 0 }}
                        />
                        <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {ep.episode_number != null && (
                            <span style={{ color: "var(--text-muted)", marginRight: 6, fontSize: 11 }}>
                              第{ep.episode_number % 1 === 0 ? ep.episode_number : ep.episode_number.toFixed(1)}话
                            </span>
                          )}
                          {ep.episode_title}
                        </span>
                        <button
                          className="btn btn-ghost"
                          style={{ fontSize: 11, padding: "2px 8px", flexShrink: 0 }}
                          onClick={() => handleStartSingle(ep.id)}
                        >
                          立即下载
                        </button>
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          )}
        </>
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
