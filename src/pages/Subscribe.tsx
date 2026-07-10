import { useNavigate } from "react-router-dom";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import PageIllustration from "../components/PageIllustration";
import Toast from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import { useToast } from "../hooks/useToast";
import { useConfirm } from "../hooks/useConfirm";
import type { RefreshResult, Subscription } from "../types";

export default function Subscribe() {
  const navigate = useNavigate();
  const [subs, setSubs] = useState<Subscription[]>([]);
  const [loading, setLoading] = useState(true);

  const [refreshing, setRefreshing] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const { toast, showToast } = useToast();
  const { request, askConfirm, resolveConfirm } = useConfirm();

  const loadSubs = useCallback(async () => {
    try {
      setLoading(true);
      const data = await invoke<Subscription[]>("get_subscriptions");
      setSubs(data);
    } catch (e) {
      showToast("加载订阅失败: " + String(e), false);
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadSubs();
  }, [loadSubs]);

  const handleRefresh = async () => {
    if (refreshing) return;
    setRefreshing(true);
    showToast("正在刷新...", true);
    try {
      const result = await invoke<RefreshResult>("refresh_all_subscriptions");
      showToast(`新增 ${result.new_episodes} 集，开始下载 ${result.started_downloads} 个`, true);
      await loadSubs();
    } catch (e) {
      showToast("刷新失败: " + String(e), false);
    } finally {
      setRefreshing(false);
    }
  };

  const handleToggleMode = async (subId: string, autoDownload: boolean) => {
    try {
      await invoke("update_auto_download", { id: subId, autoDownload });
      await loadSubs();
    } catch (e) {
      showToast("更新下载模式失败: " + String(e), false);
    }
  };
  const handleWipe = async () => {
    const confirmed = await askConfirm({
      title: "清除全部订阅",
      message: "确定要清除所有订阅和剧集记录吗？此操作不可撤销。",
      confirmLabel: "清除",
      danger: true,
    });
    if (!confirmed) return;
    try {
      const msg = await invoke<string>("wipe_all_data");
      showToast(msg, true);
      await loadSubs();
    } catch (e) {
      showToast("清除失败: " + String(e), false);
    }
  };

  return (
    <div>
      <Toast toast={toast} />
      <ConfirmDialog request={request} onResolve={resolveConfirm} />
      <div className="page-header">
        <div className="page-header-left">
          <PageIllustration page="/" />
          <div>
            <h1 className="page-title">订阅列表</h1>
            <p className="page-subtitle">管理你的番剧 RSS 订阅</p>
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
          <button className="btn btn-outline" onClick={handleRefresh} disabled={refreshing}>
            {refreshing ? "刷新中..." : "刷新全部"}
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
          <div className="card-grid">
            {subs.map((sub) => (
              <div className="card" key={sub.id}>
                <button
                  type="button"
                  className="interactive-reset"
                  aria-expanded={expandedId === sub.id}
                  onClick={() => setExpandedId(expandedId === sub.id ? null : sub.id)}
                >
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
                </button>
                {expandedId === sub.id && (
                  <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--border-color)" }}>
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
