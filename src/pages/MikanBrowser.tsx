import { useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

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
            <p className="page-subtitle">浏览番剧资源，点击番剧在容器内跳转详情</p>
          </div>
          <div style={{ marginLeft: 16, display: "flex", alignItems: "center", gap: 4 }}>
            <button style={navBtnStyle} onClick={() => navigate("back")} title="后退 (Backspace)">←</button>
            <button style={navBtnStyle} onClick={() => navigate("forward")} title="前进 (Alt+→)">→</button>
            <button style={navBtnStyle} onClick={() => navigate("reload")} title="刷新">↻</button>
            <button style={{ ...navBtnStyle, fontSize: 16 }} onClick={() => navigate("home")} title="主页">⌂</button>
          </div>
        </div>
        <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
          mikanani.me · Backspace 回退 · 鼠标滚轮滚动
        </span>
      </div>
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
