import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import PageIllustration from "../components/PageIllustration";
import Toast from "../components/Toast";
import { useToast } from "../hooks/useToast";
import type { AnimeGroup } from "../types";
import { formatEpisodeNumber } from "../utils/format";

export default function Library() {
  const [groups, setGroups] = useState<AnimeGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const { toast, showToast } = useToast();

  const loadLibrary = useCallback(async () => {
    try {
      setLoading(true);
      const data = await invoke<AnimeGroup[]>("scan_library");
      setGroups(data);
    } catch (e) {
      showToast("扫描失败: " + String(e), false);
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadLibrary();
  }, [loadLibrary]);

  const markWatched = async (filePath: string) => {
    try {
      await invoke("mark_watched", { filePath });
      await loadLibrary();
    } catch (e) {
      showToast("标记失败: " + String(e), false);
    }
  };

  const toggleExpand = (title: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(title)) next.delete(title);
      else next.add(title);
      return next;
    });
  };

  const handleOpenFile = async (filePath: string) => {
    try { await invoke("open_file", { filePath }); } catch (e) { showToast("播放失败: " + String(e), false); }
  };

  const handleOpenDir = async (filePath: string) => {
    try { await invoke("open_file_dir", { filePath }); } catch (e) { showToast("打开文件夹失败: " + String(e), false); }
  };

  return (
    <div>
      <Toast toast={toast} />
      <div className="page-header">
        <div className="page-header-left">
          <PageIllustration page="/library" />
          <div>
            <h1 className="page-title">本地番剧</h1>
            <p className="page-subtitle">管理已下载的番剧，追踪观看进度</p>
          </div>
        </div>
        <button className="btn btn-outline" onClick={loadLibrary}>
          重新扫描
        </button>
      </div>

      {loading ? (
        <div className="empty-state"><p>扫描中...</p></div>
      ) : groups.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">📂</div>
          <div className="empty-state-title">暂无本地番剧</div>
          <p>下载番剧后，此处将自动按番剧分类展示</p>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {groups.map((g) => {
            const isExpanded = expandedIds.has(g.title);
            const watchedCount = g.episodes.filter((e) => e.watched).length;
            // Use the first episode's path to locate the parent folder
            const firstEpPath = g.episodes[0]?.file_path || "";

            return (
              <div className="card" key={g.title} style={{ padding: "12px 16px" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <button
                    type="button"
                    className="interactive-reset"
                    aria-expanded={isExpanded}
                    onClick={() => toggleExpand(g.title)}
                    style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}
                  >
                    <span style={{ fontSize: 14 }}>{isExpanded ? "▾" : "▸"}</span>
                    <span style={{ fontWeight: 600, fontSize: 15, flex: 1 }}>{g.title}</span>
                    <span style={{ fontSize: 12, color: "var(--text-muted)", marginRight: 8 }}>
                      {watchedCount} / {g.episodes.length} 已看
                    </span>
                  </button>
                  <button
                    className="btn btn-ghost"
                    style={{ fontSize: 11, padding: "2px 8px", flexShrink: 0 }}
                    onClick={() => handleOpenDir(firstEpPath)}
                  >
                    打开文件夹
                  </button>
                </div>

                {/* Episode list — shown when expanded */}
                {isExpanded && (
                  <div style={{ marginTop: 8, borderTop: "1px solid var(--border-color)", paddingTop: 6 }}>
                    {[...g.episodes]
                      .sort((a, b) => (a.episode_number ?? 0) - (b.episode_number ?? 0))
                      .map((ep) => (
                        <div
                          key={ep.file_path}
                          style={{
                            display: "flex", alignItems: "center", gap: 8,
                            padding: "5px 4px", fontSize: 13,
                            borderBottom: "1px solid var(--border-color)",
                            background: ep.watched ? "var(--badge-success-bg)" : "transparent",
                            borderRadius: 4,
                          }}
                        >
                          <span style={{
                            flex: 1, minWidth: 0,
                            overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                            color: ep.watched ? "var(--text-muted)" : "var(--text-primary)",
                          }}>
                            {ep.episode_number != null && (
                              <span style={{ color: "var(--text-muted)", marginRight: 6, fontSize: 11 }}>
                                第{formatEpisodeNumber(ep.episode_number)}话
                              </span>
                            )}
                            {ep.file_name}
                          </span>
                          <button
                            className="btn btn-ghost"
                            style={{ fontSize: 11, padding: "2px 8px", flexShrink: 0 }}
                            onClick={() => handleOpenFile(ep.file_path)}
                          >
                            播放
                          </button>
                          {!ep.watched ? (
                            <button
                              className="btn btn-ghost"
                              style={{ fontSize: 11, padding: "2px 8px", flexShrink: 0 }}
                              onClick={() => markWatched(ep.file_path)}
                            >
                              标记已看
                            </button>
                          ) : (
                            <span className="badge badge-success" style={{ fontSize: 11 }}>已看</span>
                          )}
                        </div>
                      ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
