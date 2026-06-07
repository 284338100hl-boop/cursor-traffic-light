import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./Settings.css";

type AppSettings = {
  idle_ms: number;
  notify_on_complete: boolean;
};

const IDLE_MS_OPTIONS = [
  { value: 10000, label: "10秒" },
  { value: 20000, label: "20秒" },
  { value: 30000, label: "30秒" },
  { value: 60000, label: "1分钟" },
  { value: 120000, label: "2分钟" },
];

function Settings() {
  const [settings, setSettings] = useState<AppSettings>({
    idle_ms: 30000,
    notify_on_complete: false,
  });
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<AppSettings>("get_settings").then((s) => {
      setSettings(s);
    });
  }, []);

  const handleIdleMsChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newValue = parseInt(e.target.value, 10);
    const newSettings = { ...settings, idle_ms: newValue };
    setSettings(newSettings);
    saveSettings(newSettings);
  };

  const handleNotifyToggle = () => {
    const newSettings = { ...settings, notify_on_complete: !settings.notify_on_complete };
    setSettings(newSettings);
    saveSettings(newSettings);
  };

  const saveSettings = (s: AppSettings) => {
    invoke("set_settings", { settings: s }).then(() => {
      setSaved(true);
      setTimeout(() => setSaved(false), 1000);
    });
  };

  const closeWindow = () => {
    void getCurrentWindow().hide();
  };

  return (
    <div className="settings-container">
      <h2>设置</h2>

      <div className="setting-row">
        <label htmlFor="idle-ms">停顿超时时间</label>
        <select id="idle-ms" value={settings.idle_ms} onChange={handleIdleMsChange}>
          {IDLE_MS_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
        <p className="hint">黄灯超过此时间无更新将转为红灯</p>
      </div>

      <div className="setting-row">
        <div className="toggle-row">
          <label htmlFor="notify-toggle">任务完成提醒</label>
          <button
            id="notify-toggle"
            className={`toggle ${settings.notify_on_complete ? "on" : "off"}`}
            onClick={handleNotifyToggle}
            aria-pressed={settings.notify_on_complete}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">Agent完成任务时显示系统通知</p>
      </div>

      <div className="actions">
        <button className="close-btn" onClick={closeWindow}>
          关闭
        </button>
        {saved && <span className="saved-hint">已保存</span>}
      </div>
    </div>
  );
}

export default Settings;