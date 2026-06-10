import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppSettings {
  download_dir: string;
  refresh_interval: number;
  aria2_port: number;
  max_concurrent_downloads: number;
  auto_delete_torrent: boolean;
}

export default function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    download_dir: "",
    refresh_interval: 30,
    aria2_port: 6800,
    max_concurrent_downloads: 3,
    auto_delete_torrent: true,
  });
  const [saved, setSaved] = useState(false);

  const loadSettings = useCallback(async () => {
    try {
      const data = await invoke<AppSettings>("get_settings");
      setSettings(data);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSave = async () => {
    try {
      await invoke("save_settings", { settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  };

  return (
    <div style={{ maxWidth: 600 }}>
      <div className="page-header">
        <div>
          <h1 className="page-title">设置</h1>
          <p className="page-subtitle">配置下载、刷新等选项</p>
        </div>
        <button className="btn btn-primary" onClick={handleSave}>
          {saved ? "已保存" : "保存设置"}
        </button>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div className="card">
          <label style={{ fontSize: 13, fontWeight: 500, display: "block", marginBottom: 6 }}>
            下载目录
          </label>
          <input
            type="text"
            className="setting-input"
            value={settings.download_dir}
            onChange={(e) => setSettings({ ...settings, download_dir: e.target.value })}
            placeholder="默认为 bangumiao/download。RSS 订阅子文件夹、手动下载→手动下载"
            style={{
              width: "100%",
              padding: "8px 12px",
              borderRadius: 6,
              border: "1px solid var(--border-color)",
              fontSize: 13,
              outline: "none",
              fontFamily: "inherit",
            }}
          />
        </div>

        <div className="card">
          <label style={{ fontSize: 13, fontWeight: 500, display: "block", marginBottom: 6 }}>
            RSS 刷新间隔（分钟）
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.refresh_interval}
            onChange={(e) => setSettings({ ...settings, refresh_interval: parseInt(e.target.value) || 30 })}
            min={5}
            max={1440}
            style={{
              width: "100%",
              padding: "8px 12px",
              borderRadius: 6,
              border: "1px solid var(--border-color)",
              fontSize: 13,
              outline: "none",
              fontFamily: "inherit",
            }}
          />
        </div>

        <div className="card">
          <label style={{ fontSize: 13, fontWeight: 500, display: "block", marginBottom: 6 }}>
            aria2 JSON-RPC 端口
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.aria2_port}
            onChange={(e) => setSettings({ ...settings, aria2_port: parseInt(e.target.value) || 6800 })}
            style={{
              width: "100%",
              padding: "8px 12px",
              borderRadius: 6,
              border: "1px solid var(--border-color)",
              fontSize: 13,
              outline: "none",
              fontFamily: "inherit",
            }}
          />
        </div>

        <div className="card">
          <label style={{ fontSize: 13, fontWeight: 500, display: "block", marginBottom: 6 }}>
            最大同时下载数
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.max_concurrent_downloads}
            onChange={(e) => setSettings({ ...settings, max_concurrent_downloads: parseInt(e.target.value) || 3 })}
            min={1}
            max={10}
            style={{
              width: "100%",
              padding: "8px 12px",
              borderRadius: 6,
              border: "1px solid var(--border-color)",
              fontSize: 13,
              outline: "none",
              fontFamily: "inherit",
            }}
          />
        </div>

        <div className="card" style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <label style={{ fontSize: 13, fontWeight: 500 }}>
            下载后自动删除种子文件
          </label>
          <input
            type="checkbox"
            checked={settings.auto_delete_torrent}
            onChange={(e) => setSettings({ ...settings, auto_delete_torrent: e.target.checked })}
            style={{ width: 18, height: 18, cursor: "pointer" }}
          />
        </div>
      </div>
    </div>
  );
}
