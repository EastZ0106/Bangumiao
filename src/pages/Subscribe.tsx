import { useNavigate } from "react-router-dom";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import PageIllustration from "../components/PageIllustration";

interface Subscription {
  id: string;
  title: string;
  rss_url: string;
  mikan_url: string;
  cover_url: string;
  enabled: boolean;
  auto_download: boolean;
  created_at: string;
}

export default function Subscribe() {
  const navigate = useNavigate();
  const [subs, setSubs] = useState<Subscription[]>([]);
  const [loading, setLoading] = useState(true);

  const [refreshMsg, setRefreshMsg] = useState("");
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const loadSubs = useCallback(async () => {
    try {
      setLoading(true);
      const data = await invoke<Subscription[]>("get_subscriptions");
      setSubs(data);
    } catch (e) {
      console.error("Failed to load subscriptions:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSubs();
  }, [loadSubs]);

  const handleRefresh = async () => {
    setRefreshMsg("正在刷新...");
    try {
      const result = await invoke<{ new_episodes: number; started_downloads: number }>("refresh_all_subscriptions");
      setRefreshMsg(`新增 ${result.new_episodes} 集，开始下载 ${result.started_downloads} 个`);
      await loadSubs();
      setTimeout(() => setRefreshMsg(""), 4000);
    } catch (e) {
      setRefreshMsg("刷新失败: " + String(e));
      setTimeout(() => setRefreshMsg(""), 4000);
    }
  };

  const handleToggleMode = async (subId: string, autoDownload: boolean) => {
    try {
      await invoke("update_auto_download", { id: subId, autoDownload });
      await loadSubs();
    } catch (e) {
      console.error("Failed to update mode:", e);
    }
  };
  const handleWipe = async () => {
    if (!confirm("确定要清除所有订阅和剧集记录吗？此操作不可撤销。")) return;
    try {
      const msg = await invoke<string>("wipe_all_data");
      setRefreshMsg(msg);
      setTimeout(() => setRefreshMsg(""), 3000);
      await loadSubs();
    } catch (e) {
      setRefreshMsg("清除失败: " + String(e));
      setTimeout(() => setRefreshMsg(""), 4000);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div className="page-header-left">
          <PageIllustration page="/" />
          <div>
            <h1 className="page-title">订阅列表</h1>
            <p className="page-subtitle">管理你的番剧 RSS 订阅</p>
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
          <button className="btn btn-outline" onClick={handleRefresh}>
            刷新全部
          </button>
          <button className="btn btn-outline" onClick={handleWipe} style={{ color: "var(--color-error)" }}>
            清除全部
          </button>
          <button className="btn btn-primary" onClick={() => navigate("/browse")}>
            添加订阅
          </button>
        </div>
      </div>

      {loading ? (
        <div className="empty-state">
          <p>加载中...</p>
        </div>
      ) : subs.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">📡</div>
          <div className="empty-state-title">还没有订阅任何番剧</div>
          <p>前往「蜜柑计划」浏览并添加订阅</p>
        </div>
      ) : (
        <>
          {refreshMsg && (
            <div style={{
              marginBottom: 12, padding: "8px 12px", borderRadius: 6, fontSize: 12,
              background: refreshMsg.includes("失败") ? "var(--toast-error-bg)" : "var(--toast-success-bg)",
              color: refreshMsg.includes("失败") ? "var(--toast-error-text)" : "var(--toast-success-text)",
            }}>
              {refreshMsg}
            </div>
          )}
          <div className="card-grid">
            {subs.map((sub) => (
              <div className="card" key={sub.id} onClick={() => setExpandedId(expandedId === sub.id ? null : sub.id)} style={{ cursor: "pointer" }}>
                <h3 style={{ fontSize: 15, marginBottom: 8 }}>{sub.title}</h3>
                <div style={{ fontSize: 12, color: "var(--text-muted)", display: "flex", flexWrap: "wrap", gap: 6 }}>
                  {sub.enabled ? (
                    <span className="badge badge-success">启用</span>
                  ) : (
                    <span className="badge badge-warning">暂停</span>
                  )}
                  <span className="badge" style={{ background: sub.auto_download ? "var(--color-primary-500)" : "var(--color-warning)", color: "#FFFCF7" }}>
                    {sub.auto_download ? "自动下载" : "手动管理"}
                  </span>
                </div>
                {expandedId === sub.id && (
                  <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--border-color)" }} onClick={(e) => e.stopPropagation()}>
                    <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 8 }}>下载模式</div>
                    <div style={{ display: "flex", gap: 8 }}>
                      <button
                        className={`btn ${sub.auto_download ? "btn-primary" : "btn-ghost"}`}
                        style={{ fontSize: 12, padding: "4px 12px" }}
                        onClick={() => handleToggleMode(sub.id, true)}
                      >
                        自动下载
                      </button>
                      <button
                        className={`btn ${!sub.auto_download ? "btn-primary" : "btn-ghost"}`}
                        style={{ fontSize: 12, padding: "4px 12px" }}
                        onClick={() => handleToggleMode(sub.id, false)}
                      >
                        手动管理
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
