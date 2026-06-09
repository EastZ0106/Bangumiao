import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AnimeGroup {
  title: string;
  episodes: EpisodeItem[];
}

interface EpisodeItem {
  file_path: string;
  episode_number: number | null;
  episode_title: string;
  downloaded: boolean;
  watched: boolean;
  file_name: string;
}

export default function Library() {
  const [groups, setGroups] = useState<AnimeGroup[]>([]);
  const [loading, setLoading] = useState(true);

  const loadLibrary = useCallback(async () => {
    try {
      setLoading(true);
      const data = await invoke<AnimeGroup[]>("scan_library");
      setGroups(data);
    } catch (e) {
      console.error("Failed to scan library:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadLibrary();
  }, [loadLibrary]);

  const markWatched = async (filePath: string) => {
    try {
      await invoke("mark_watched", { filePath });
      await loadLibrary();
    } catch (e) {
      console.error("Failed to mark watched:", e);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">本地番剧</h1>
          <p className="page-subtitle">管理已下载的番剧，追踪观看进度</p>
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
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {groups.map((g) => (
            <div className="card" key={g.title}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
                <h3 style={{ fontSize: 16, fontWeight: 600 }}>{g.title}</h3>
                <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  {g.episodes.filter((e) => e.watched).length} / {g.episodes.length} 已看
                </span>
              </div>
              <div style={{ fontSize: 12 }}>
                {g.episodes
                  .sort((a, b) => (a.episode_number ?? 0) - (b.episode_number ?? 0))
                  .map((ep) => (
                    <div
                      key={ep.file_path}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        padding: "6px 8px",
                        borderRadius: 6,
                        background: ep.watched ? "var(--color-primary-50)" : "transparent",
                      }}
                    >
                      <span style={{ flex: 1, color: ep.watched ? "var(--text-muted)" : "var(--text-primary)" }}>
                        {ep.episode_number != null ? `第 ${ep.episode_number} 话` : ""} — {ep.file_name}
                      </span>
                      {!ep.watched && (
                        <button
                          className="btn btn-ghost"
                          style={{ fontSize: 12, padding: "3px 10px" }}
                          onClick={() => markWatched(ep.file_path)}
                        >
                          标记已看
                        </button>
                      )}
                      {ep.watched && <span className="badge badge-success">已看</span>}
                    </div>
                  ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
