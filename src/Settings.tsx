import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./Settings.css";

type AppSettings = {
  idle_ms: number;
  notify_on_complete: boolean;
  notify_ntfy: boolean;
  ntfy_topic: string;
  always_on_top: boolean;
  show_hover_tooltip: boolean;
  auto_launch_with_cursor: boolean;
  toast_text: string;
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
    notify_ntfy: true,
    ntfy_topic: "",
    always_on_top: true,
    show_hover_tooltip: true,
    auto_launch_with_cursor: false,
    toast_text: "状态变化",
  });
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      const s = await invoke<AppSettings>("get_settings");
      if (!cancelled) setSettings(s);

      unlisten = await listen<AppSettings>("settings-changed", (event) => {
        if (!cancelled) setSettings(event.payload);
      });
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const saveSettings = (s: AppSettings) => {
    invoke("set_settings", { settings: s }).then(() => {
      setSaved(true);
      setTimeout(() => setSaved(false), 1000);
    });
  };

  const patchSettings = (patch: Partial<AppSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    saveSettings(next);
  };

  const handleIdleMsChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    patchSettings({ idle_ms: parseInt(e.target.value, 10) });
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
          <label htmlFor="always-on-top-toggle">窗口置顶</label>
          <button
            id="always-on-top-toggle"
            className={`toggle ${settings.always_on_top ? "on" : "off"}`}
            onClick={() => patchSettings({ always_on_top: !settings.always_on_top })}
            aria-pressed={settings.always_on_top}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">红绿灯窗口始终显示在其他窗口之上</p>
      </div>

      <div className="setting-row">
        <div className="toggle-row">
          <label htmlFor="hover-tooltip-toggle">悬停显示对话名称</label>
          <button
            id="hover-tooltip-toggle"
            className={`toggle ${settings.show_hover_tooltip ? "on" : "off"}`}
            onClick={() =>
              patchSettings({ show_hover_tooltip: !settings.show_hover_tooltip })
            }
            aria-pressed={settings.show_hover_tooltip}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">鼠标悬停红绿灯时显示当前对话标题</p>
      </div>

      <div className="setting-row">
        <label htmlFor="toast-text">贴边通知文字</label>
        <input
          id="toast-text"
          className="text-input toast-text-input"
          type="text"
          value={settings.toast_text}
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => setSettings({ ...settings, toast_text: e.target.value })}
          onBlur={(e) => {
            const toast_text = e.target.value.trim();
            const next = { ...settings, toast_text };
            setSettings(next);
            saveSettings(next);
          }}
        />
        <p className="hint">贴边缩小后，灯色变化时显示这段文字</p>
      </div>

      <div className="setting-row">
        <div className="toggle-row">
          <label htmlFor="auto-launch-toggle">Cursor 启动时自动打开</label>
          <button
            id="auto-launch-toggle"
            className={`toggle ${settings.auto_launch_with_cursor ? "on" : "off"}`}
            onClick={() =>
              patchSettings({
                auto_launch_with_cursor: !settings.auto_launch_with_cursor,
              })
            }
            aria-pressed={settings.auto_launch_with_cursor}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">Cursor 打开工作区时自动启动红绿灯（需先手动运行过一次）</p>
      </div>

      <div className="setting-row">
        <div className="toggle-row">
          <label htmlFor="notify-toggle">任务完成提醒</label>
          <button
            id="notify-toggle"
            className={`toggle ${settings.notify_on_complete ? "on" : "off"}`}
            onClick={() =>
              patchSettings({ notify_on_complete: !settings.notify_on_complete })
            }
            aria-pressed={settings.notify_on_complete}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">主对话任务完成时显示电脑系统通知</p>
      </div>

      <div className="setting-row">
        <div className="toggle-row">
          <label htmlFor="ntfy-toggle">手机 ntfy 提醒</label>
          <button
            id="ntfy-toggle"
            className={`toggle ${settings.notify_ntfy ? "on" : "off"}`}
            onClick={() => patchSettings({ notify_ntfy: !settings.notify_ntfy })}
            aria-pressed={settings.notify_ntfy}
          >
            <span className="toggle-knob" />
          </button>
        </div>
        <p className="hint">完成时推送「任务完成」，红灯时推送「有问题需要处理」</p>
        <input
          id="ntfy-topic"
          className="text-input"
          type="text"
          value={settings.ntfy_topic}
          placeholder="ntfy topic"
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => setSettings({ ...settings, ntfy_topic: e.target.value })}
          onBlur={(e) => {
            const ntfy_topic = e.target.value.trim();
            const next = { ...settings, ntfy_topic };
            setSettings(next);
            saveSettings(next);
          }}
        />
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
