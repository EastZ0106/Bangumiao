import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import PageIllustration from "../components/PageIllustration";
import Toast from "../components/Toast";
import { useToast } from "../hooks/useToast";
import type { AppSettings } from "../types";

export default function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    download_dir: "",
    refresh_interval: 30,
    aria2_port: 6800,
    max_concurrent_downloads: 3,
    auto_delete_torrent: true,
    close_to_tray: true,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const { toast, showToast } = useToast();

  const clamp = (value: number, min: number, max: number) => {
    if (!Number.isFinite(value)) return min;
    return Math.min(max, Math.max(min, value));
  };

  const setNumberSetting = (key: keyof Pick<AppSettings, "refresh_interval" | "aria2_port" | "max_concurrent_downloads">, value: string) => {
    const parsed = Number.parseInt(value, 10);
    setSettings(prev => ({ ...prev, [key]: Number.isNaN(parsed) ? 0 : parsed }));
  };

  const loadSettings = useCallback(async () => {
    try {
      setLoading(true);
      const data = await invoke<AppSettings>("get_settings");
      setSettings(data);
    } catch (e) {
      showToast("加载设置失败: " + String(e), false);
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSave = async () => {
    const normalized: AppSettings = {
      ...settings,
      refresh_interval: clamp(settings.refresh_interval, 1, 1440),
      aria2_port: clamp(settings.aria2_port, 1024, 65535),
      max_concurrent_downloads: clamp(settings.max_concurrent_downloads, 1, 10),
    };
    setSaving(true);
    try {
      await invoke("save_settings", { settings: normalized });
      setSettings(normalized);
      showToast("设置已保存，下载服务已按需重启", true);
    } catch (e) {
      showToast("保存失败: " + String(e), false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={{ maxWidth: 600 }}>
      <Toast toast={toast} />
      <div className="page-header">
        <div className="page-header-left">
          <PageIllustration page="/settings" />
          <div>
            <h1 className="page-title">设置</h1>
            <p className="page-subtitle">配置下载、刷新等选项</p>
          </div>
        </div>
        <button className="btn btn-primary" onClick={handleSave} disabled={loading || saving}>
          {saving ? "保存中..." : "保存设置"}
        </button>
      </div>

      <div className="settings-stack">
        <div className="card">
          <label className="setting-label">
            下载目录
          </label>
          <input
            type="text"
            className="setting-input"
            value={settings.download_dir}
            onChange={(e) => setSettings({ ...settings, download_dir: e.target.value })}
            disabled={loading}
            placeholder="默认为 bangumiao/download。RSS 订阅子文件夹、手动下载→手动下载"
          />
        </div>

        <div className="card">
          <label className="setting-label">
            RSS 刷新间隔（分钟）
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.refresh_interval || ""}
            onChange={(e) => setNumberSetting("refresh_interval", e.target.value)}
            disabled={loading}
            min={1}
            max={1440}
          />
        </div>

        <div className="card">
          <label className="setting-label">
            aria2 JSON-RPC 端口
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.aria2_port || ""}
            onChange={(e) => setNumberSetting("aria2_port", e.target.value)}
            disabled={loading}
            min={1024}
            max={65535}
          />
        </div>

        <div className="card">
          <label className="setting-label">
            最大同时下载数
          </label>
          <input
            type="number"
            className="setting-input"
            value={settings.max_concurrent_downloads || ""}
            onChange={(e) => setNumberSetting("max_concurrent_downloads", e.target.value)}
            disabled={loading}
            min={1}
            max={10}
          />
        </div>

        <div className="card setting-toggle">
          <label style={{ fontSize: 13, fontWeight: 500 }}>
            下载后自动删除种子文件
          </label>
          <input
            type="checkbox"
            checked={settings.auto_delete_torrent}
            onChange={(e) => setSettings({ ...settings, auto_delete_torrent: e.target.checked })}
            disabled={loading}
            style={{ width: 18, height: 18, cursor: "pointer" }}
          />
        </div>

        <div className="card setting-toggle">
          <div>
            <label style={{ fontSize: 13, fontWeight: 500 }}>
              关闭窗口最小化到系统托盘
            </label>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              点击窗口关闭按钮时隐藏到托盘图标而非退出程序
            </p>
          </div>
          <input
            type="checkbox"
            checked={settings.close_to_tray}
            onChange={(e) => setSettings({ ...settings, close_to_tray: e.target.checked })}
            disabled={loading}
            style={{ width: 18, height: 18, cursor: "pointer" }}
          />
        </div>
      </div>
    </div>
  );
}
