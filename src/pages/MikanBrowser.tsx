import { useRef, useEffect, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface RssCandidate {
  anime_title: string;
  subgroup_name: string;
  rss_url: string;
  bangumi_id: string;
  subgroup_id: string;
}

export default function MikanBrowser() {
  const containerRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    let cancelled = false;
    const open = async () => {
      if (!containerRef.current || cancelled) return;
      const rect = containerRef.current.getBoundingClientRect();
      await invoke("open_mikan_browser", {
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
      }).catch((e) => console.error("Failed to open mikan browser:", e));
    };

    const raf = requestAnimationFrame(() => { open(); });

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
  }, [updateBounds]);

  // RSS scan state
  const [scanning, setScanning] = useState(false);
  const [candidates, setCandidates] = useState<RssCandidate[]>([]);
  const [showModal, setShowModal] = useState(false);
  const [subscribing, setSubscribing] = useState<string | null>(null);
  const [scanMsg, setScanMsg] = useState("");

  const handleScanRss = async () => {
    setScanning(true);
    setScanMsg("正在扫描页面中的 RSS 链接...");
    try {
      const result = await invoke<RssCandidate[]>("scan_mikan_rss");
      if (result.length === 0) {
        setScanMsg("未发现 RSS 链接。请确保当前浏览的是一个番剧的详情页或字幕组列表页。");
        setCandidates([]);
      } else {
        setCandidates(result);
        setShowModal(true);
        setScanMsg("");
      }
    } catch (e) {
      setScanMsg("扫描失败: " + String(e));
      setCandidates([]);
    } finally {
      setScanning(false);
    }
  };

  const handleSubscribe = async (c: RssCandidate) => {
    setSubscribing(c.rss_url);
    try {
      await invoke("add_subscription", {
        rssUrl: c.rss_url,
        title: c.anime_title + " - " + c.subgroup_name,
        mikanUrl: "https://mikanani.me/Home/Bangumi/" + c.bangumi_id,
      });
      setSubscribing(null);
      // Remove from list so user knows it's done
      setCandidates((prev) => prev.filter((x) => x.rss_url !== c.rss_url));
      if (candidates.length <= 1) {
        setShowModal(false);
      }
    } catch (e) {
      setSubscribing(null);
      setScanMsg("订阅失败: " + String(e));
    }
  };

  const navBtnStyle: React.CSSProperties = {
    display: "inline-flex", alignItems: "center", justifyContent: "center",
    width: 28, height: 28, borderRadius: 6,
    border: "1px solid var(--border-color)", background: "var(--bg-card)",
    cursor: "pointer", fontSize: 13, color: "var(--text-secondary)",
  };

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div>
            <h1 className="page-title">蜜柑计划</h1>
            <p className="page-subtitle">浏览番剧资源，一键抓取 RSS 订阅</p>
          </div>
          <div style={{ marginLeft: 16, display: "flex", alignItems: "center", gap: 4 }}>
            <button style={navBtnStyle} onClick={() => navigate("back")} title="后退 (Backspace)">←</button>
            <button style={navBtnStyle} onClick={() => navigate("forward")} title="前进 (Alt+→)">→</button>
            <button style={navBtnStyle} onClick={() => navigate("reload")} title="刷新">↻</button>
            <button style={{ ...navBtnStyle, fontSize: 16 }} onClick={() => navigate("home")} title="主页">⌂</button>
          </div>
          <button
            className="btn btn-primary"
            onClick={handleScanRss}
            disabled={scanning}
            style={{ marginLeft: 12, fontSize: 12, padding: "6px 12px" }}
          >
            {scanning ? "扫描中..." : "🔗 抓取 RSS"}
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

      {/* RSS Candidate Modal */}
      {showModal && candidates.length > 0 && (
        <div style={{
          position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
          background: "rgba(0,0,0,0.4)", display: "flex", alignItems: "center", justifyContent: "center",
          zIndex: 1000,
        }} onClick={() => setShowModal(false)}>
          <div style={{
            background: "var(--bg-card)", borderRadius: 12, padding: 24, maxWidth: 500, width: "90%",
            maxHeight: "70vh", overflow: "auto", boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
          }} onClick={(e) => e.stopPropagation()}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
              <h3 style={{ fontSize: 16, fontWeight: 600 }}>找到 {candidates.length} 个 RSS 源</h3>
              <button onClick={() => setShowModal(false)} style={{
                width: 28, height: 28, borderRadius: 6, border: "none", cursor: "pointer",
                background: "transparent", fontSize: 16, color: "var(--text-secondary)",
              }}>✕</button>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {candidates.map((c) => (
                <div key={c.rss_url} style={{
                  display: "flex", justifyContent: "space-between", alignItems: "center",
                  padding: "10px 12px", borderRadius: 8, border: "1px solid var(--border-color)",
                  background: "var(--bg-sidebar)",
                }}>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 13, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {c.anime_title}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>
                      {c.subgroup_name}
                    </div>
                  </div>
                  <button
                    className="btn btn-primary"
                    onClick={() => handleSubscribe(c)}
                    disabled={subscribing === c.rss_url}
                    style={{ fontSize: 11, padding: "4px 10px", flexShrink: 0, marginLeft: 8 }}
                  >
                    {subscribing === c.rss_url ? "订阅中..." : "订阅"}
                  </button>
                </div>
              ))}
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
