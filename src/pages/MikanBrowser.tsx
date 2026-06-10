import { useRef, useEffect, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SubgroupInfo {
  subgroup_id: string;
  subgroup_name: string;
  anime_title: string;
  bangumi_id: string;
}

interface RssEpisode {
  title: string;
  torrent_url: string;
  magnet_uri: string;
  pub_date: string;
  episode_number: number | null;
}

export default function MikanBrowser() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [webviewOpen, setWebviewOpen] = useState(false);

  const navigate = useCallback((action: "back" | "forward" | "reload" | "home") => {
    const jsMap: Record<string, string> = {
      back: "history.back()",
      forward: "history.forward()",
      reload: "location.reload()",
      home: "location.href='https://mikanani.me'",
    };
    invoke("mikan_eval", { js: jsMap[action] }).catch(() => {});
  }, []);

  const updateBounds = useCallback(async () => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    await invoke("update_mikan_browser_bounds", {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    }).catch(() => {});
  }, []);

  // Helpers to open/close the native child WebView
  const openWebView = useCallback(async () => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    await invoke("open_mikan_browser", {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    }).catch((e) => console.error("Failed to open mikan browser:", e));
    setWebviewOpen(true);
  }, []);

  const closeWebView = useCallback(async () => {
    await invoke("close_mikan_browser").catch(() => {});
    setWebviewOpen(false);
  }, []);

  // Initial open + resize tracking
  useEffect(() => {
    let cancelled = false;
    const raf = requestAnimationFrame(async () => {
      if (!cancelled) await openWebView();
    });

    const onResize = () => { updateBounds(); };
    window.addEventListener("resize", onResize);

    const observer = new ResizeObserver(() => { updateBounds(); });
    if (containerRef.current) observer.observe(containerRef.current);

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
      observer.disconnect();
      invoke("close_mikan_browser").catch(() => {});
    };
  }, [updateBounds, openWebView]);

  // ── Scan state ──
  const [scanning, setScanning] = useState(false);
  const [scanMsg, setScanMsg] = useState("");

  // Step 1: subgroup list from page scan
  const [subgroups, setSubgroups] = useState<SubgroupInfo[]>([]);
  const [showSubgroupModal, setShowSubgroupModal] = useState(false);

  // Step 2: episode preview for a selected subgroup
  const [episodes, setEpisodes] = useState<RssEpisode[]>([]);
  const [showEpisodeModal, setShowEpisodeModal] = useState(false);
  const [selectedSubgroup, setSelectedSubgroup] = useState<SubgroupInfo | null>(null);
  const [fetchingEpisodes, setFetchingEpisodes] = useState(false);
  const [subscribing, setSubscribing] = useState(false);

  // Close webview whenever ANY modal opens; reopen when all modals close
  const isAnyModalOpen = showSubgroupModal || showEpisodeModal;

  useEffect(() => {
    if (isAnyModalOpen && webviewOpen) {
      closeWebView();
    } else if (!isAnyModalOpen && !webviewOpen) {
      // Small delay so the DOM updates before we query container bounds
      const t = setTimeout(() => { openWebView(); }, 100);
      return () => clearTimeout(t);
    }
  }, [isAnyModalOpen, webviewOpen, closeWebView, openWebView]);

  /// Step 1: scan the page for all subgroups
  const handleScanRss = async () => {
    setScanning(true);
    setScanMsg("正在扫描页面中的字幕组...");
    try {
      const result = await invoke<SubgroupInfo[]>("scan_mikan_rss");
      if (result.length === 0) {
        setScanMsg("未发现字幕组。请确保当前浏览的是一个番剧详情页。");
        setSubgroups([]);
      } else {
        setSubgroups(result);
        setShowSubgroupModal(true);
        setScanMsg("");
      }
    } catch (e) {
      setScanMsg("扫描失败: " + String(e));
      setSubgroups([]);
    } finally {
      setScanning(false);
    }
  };

  /// Step 2: fetch RSS episodes for the chosen subgroup
  const handleSelectSubgroup = async (sg: SubgroupInfo) => {
    setSelectedSubgroup(sg);
    setShowSubgroupModal(false);
    setFetchingEpisodes(true);
    setScanMsg(`正在获取 "${sg.subgroup_name}" 的 RSS 内容...`);
    try {
      const eps = await invoke<RssEpisode[]>("fetch_mikan_rss", {
        bangumiId: sg.bangumi_id,
        subgroupId: sg.subgroup_id,
      });
      setEpisodes(eps);
      setShowEpisodeModal(true);
      setScanMsg("");
    } catch (e) {
      setScanMsg("获取 RSS 失败: " + String(e));
      setEpisodes([]);
    } finally {
      setFetchingEpisodes(false);
    }
  };

  /// Step 3: subscribe to the selected subgroup
  const handleSubscribe = async () => {
    if (!selectedSubgroup) return;
    setSubscribing(true);
    try {
      const rssUrl = `https://mikanani.me/RSS/Bangumi?bangumiId=${selectedSubgroup.bangumi_id}&subgroupid=${selectedSubgroup.subgroup_id}`;
      await invoke("add_subscription", {
        rssUrl,
        title: selectedSubgroup.anime_title + " - " + selectedSubgroup.subgroup_name,
        mikanUrl: "https://mikanani.me/Home/Bangumi/" + selectedSubgroup.bangumi_id,
      });
      setShowEpisodeModal(false);
      setScanMsg(`已订阅: ${selectedSubgroup.anime_title} [${selectedSubgroup.subgroup_name}]，正在拉取 RSS...`);
      // Automatically trigger a refresh so episodes get pulled in immediately
      try {
        const r = await invoke<{ new_episodes: number; started_downloads: number }>("refresh_all_subscriptions");
        setScanMsg(`已订阅 · 新增 ${r.new_episodes} 集，开始下载 ${r.started_downloads} 个`);
      } catch {
        setScanMsg(`已订阅: ${selectedSubgroup.anime_title} [${selectedSubgroup.subgroup_name}]`);
      }
      setTimeout(() => setScanMsg(""), 5000);
    } catch (e) {
      setScanMsg("订阅失败: " + String(e));
    } finally {
      setSubscribing(false);
    }
  };

  const navBtnStyle: React.CSSProperties = {
    display: "inline-flex", alignItems: "center", justifyContent: "center",
    width: 28, height: 28, borderRadius: 6,
    border: "1px solid var(--border-color)", background: "var(--bg-card)",
    cursor: "pointer", fontSize: 13, color: "var(--text-secondary)",
  };

  const formatPubDate = (d: string) => {
    try {
      const date = new Date(d);
      if (isNaN(date.getTime())) return d;
      return date.toLocaleString("zh-CN", {
        month: "2-digit", day: "2-digit",
        hour: "2-digit", minute: "2-digit",
      });
    } catch {
      return d;
    }
  };

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div>
            <h1 className="page-title">蜜柑计划</h1>
            <p className="page-subtitle">浏览番剧资源，选择字幕组订阅 RSS</p>
          </div>
          <div style={{ marginLeft: 16, display: "flex", alignItems: "center", gap: 4 }}>
            <button style={navBtnStyle} onClick={() => navigate("back")} title="后退">←</button>
            <button style={navBtnStyle} onClick={() => navigate("forward")} title="前进">→</button>
            <button style={navBtnStyle} onClick={() => navigate("reload")} title="刷新">↻</button>
            <button style={{ ...navBtnStyle, fontSize: 16 }} onClick={() => navigate("home")} title="主页">⌂</button>
          </div>
          <button
            className="btn btn-primary"
            onClick={handleScanRss}
            disabled={scanning || fetchingEpisodes}
            style={{ marginLeft: 12, fontSize: 12, padding: "6px 12px" }}
          >
            {scanning ? "扫描中..." : fetchingEpisodes ? "获取中..." : "🔗 抓取 RSS"}
          </button>
        </div>
        <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
          mikanani.me
        </span>
      </div>

      {scanMsg && (
        <div style={{
          marginBottom: 12, padding: "8px 12px", borderRadius: 6, fontSize: 12,
          background: scanMsg.includes("失败") ? "#fee2e2" : "#dbeafe",
          color: scanMsg.includes("失败") ? "#991b1b" : "#1e40af",
        }}>
          {scanMsg}
        </div>
      )}

      {/* ── Step 1 modal: pick a subgroup ── */}
      {showSubgroupModal && subgroups.length > 0 && (
        <div style={{
          position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
          background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 1000,
        }} onClick={() => setShowSubgroupModal(false)}>
          <div style={{
            background: "var(--bg-card)", borderRadius: 12, padding: 24, maxWidth: 520, width: "90%",
            maxHeight: "70vh", overflow: "auto", boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
          }} onClick={(e) => e.stopPropagation()}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
              <h3 style={{ fontSize: 16, fontWeight: 600 }}>选择字幕组</h3>
              <button onClick={() => setShowSubgroupModal(false)} style={{
                width: 28, height: 28, borderRadius: 6, border: "none", cursor: "pointer",
                background: "transparent", fontSize: 16, color: "var(--text-secondary)",
              }}>✕</button>
            </div>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
              番剧: {subgroups[0]?.anime_title} · Bangumi ID: {subgroups[0]?.bangumi_id} · 共 {subgroups.length} 个字幕组
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {subgroups.map((sg) => (
                <div key={sg.subgroup_id} style={{
                  display: "flex", justifyContent: "space-between", alignItems: "center",
                  padding: "10px 12px", borderRadius: 8, border: "1px solid var(--border-color)",
                  background: "var(--bg-sidebar)", cursor: "pointer",
                }} onClick={() => handleSelectSubgroup(sg)}>
                  <span style={{ fontSize: 13, fontWeight: 500 }}>{sg.subgroup_name}</span>
                  <span style={{ fontSize: 11, color: "var(--text-muted)" }}>查看剧集 →</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* ── Step 2 modal: preview episodes, then subscribe ── */}
      {showEpisodeModal && selectedSubgroup && (
        <div style={{
          position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
          background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 1000,
        }} onClick={() => setShowEpisodeModal(false)}>
          <div style={{
            background: "var(--bg-card)", borderRadius: 12, padding: 24, maxWidth: 560, width: "90%",
            maxHeight: "75vh", overflow: "auto", boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
          }} onClick={(e) => e.stopPropagation()}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 12 }}>
              <div>
                <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 2 }}>{selectedSubgroup.anime_title}</h3>
                <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {selectedSubgroup.subgroup_name} · 共 {episodes.length} 集
                </p>
              </div>
              <button onClick={() => setShowEpisodeModal(false)} style={{
                width: 28, height: 28, borderRadius: 6, border: "none", cursor: "pointer",
                background: "transparent", fontSize: 16, color: "var(--text-secondary)",
              }}>✕</button>
            </div>

            {/* Episode list */}
            <div style={{
              display: "flex", flexDirection: "column", gap: 4,
              maxHeight: "40vh", overflow: "auto", marginBottom: 16,
            }}>
              {episodes.map((ep, i) => (
                <div key={i} style={{
                  display: "flex", justifyContent: "space-between", alignItems: "center",
                  padding: "6px 10px", borderRadius: 6, fontSize: 12,
                  background: i % 2 === 0 ? "var(--bg-sidebar)" : "transparent",
                }}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <span style={{
                      overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", display: "block",
                    }}>
                      {ep.episode_number != null && (
                        <span style={{ color: "var(--text-muted)", marginRight: 6 }}>
                          第{ep.episode_number % 1 === 0 ? ep.episode_number : ep.episode_number.toFixed(1)}话
                        </span>
                      )}
                      {ep.title}
                    </span>
                  </div>
                  <span style={{ color: "var(--text-muted)", flexShrink: 0, marginLeft: 12 }}>
                    {formatPubDate(ep.pub_date)}
                  </span>
                </div>
              ))}
            </div>

            {/* Action buttons */}
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button
                className="btn btn-ghost"
                onClick={() => {
                  setShowEpisodeModal(false);
                  setShowSubgroupModal(true);
                }}
                style={{ fontSize: 12, padding: "6px 14px" }}
              >
                返回字幕组列表
              </button>
              <button
                className="btn btn-primary"
                onClick={handleSubscribe}
                disabled={subscribing}
                style={{ fontSize: 12, padding: "6px 14px" }}
              >
                {subscribing ? "订阅中..." : `订阅 ${selectedSubgroup.subgroup_name}`}
              </button>
            </div>
          </div>
        </div>
      )}

      <div
        ref={containerRef}
        style={{
          flex: 1, borderRadius: "var(--border-radius)", overflow: "hidden",
          border: "1px solid var(--border-color)", background: "#1a1a2e",
        }}
      />
    </div>
  );
}
