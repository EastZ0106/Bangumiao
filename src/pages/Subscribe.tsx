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
    try {
      await invoke("refresh_all_subscriptions");
      await loadSubs();
    } catch (e) {
      console.error("Refresh failed:", e);
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
      )}
    </div>
  );
}
