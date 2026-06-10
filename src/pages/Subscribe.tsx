import { useNavigate } from "react-router-dom";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Subscription {
  id: string;
  title: string;
  rss_url: string;
  mikan_url: string;
  cover_url: string;
  enabled: boolean;
  created_at: string;
}

export default function Subscribe() {
  const navigate = useNavigate();
  const [subs, setSubs] = useState<Subscription[]>([]);
  const [loading, setLoading] = useState(true);

  const [refreshMsg, setRefreshMsg] = useState("");

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
        <div>
          <h1 className="page-title">订阅列表</h1>
          <p className="page-subtitle">管理你的番剧 RSS 订阅</p>
        </div>
        <div className="page-actions" style={{ display: "flex", gap: 8 }}>
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
              background: refreshMsg.includes("失败") ? "#fee2e2" : "#dbeafe",
              color: refreshMsg.includes("失败") ? "#991b1b" : "#1e40af",
            }}>
              {refreshMsg}
            </div>
          )}
          <div className="card-grid">
            {subs.map((sub) => (
              <div className="card" key={sub.id}>
                <h3 style={{ fontSize: 15, marginBottom: 8 }}>{sub.title}</h3>
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  {sub.enabled ? (
                    <span className="badge badge-success">启用</span>
                  ) : (
                    <span className="badge badge-warning">暂停</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
