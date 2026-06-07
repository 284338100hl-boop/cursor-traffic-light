import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

type LightState = "red" | "yellow" | "green";

type TrafficLightPayload = {
  light: string;
  updatedAt?: number;
  label?: string;
  blink?: boolean;
};

type AppSettings = {
  idle_ms: number;
  notify_on_complete: boolean;
};

const DEFAULT_IDLE_MS = 30_000;

const LIGHTS: LightState[] = ["red", "yellow", "green"];

const LIGHT_STYLE: Record<
  LightState,
  { on: string; off: string; glow: string }
> = {
  red: {
    on: "#ef4444",
    off: "#3f1f1f",
    glow: "0 0 32px rgba(239, 68, 68, 0.6), inset 0 0 16px rgba(255, 255, 255, 0.2)",
  },
  yellow: {
    on: "#facc15",
    off: "#3f3f1f",
    glow: "0 0 32px rgba(250, 204, 21, 0.6), inset 0 0 16px rgba(255, 255, 255, 0.2)",
  },
  green: {
    on: "#22c55e",
    off: "#1f3f2f",
    glow: "0 0 32px rgba(34, 197, 94, 0.6), inset 0 0 16px rgba(255, 255, 255, 0.2)",
  },
};

const ARIA: Record<LightState, string> = {
  red: "红灯",
  yellow: "黄灯",
  green: "绿灯",
};

function normalizeLight(value: string | undefined): LightState {
  if (value === "red" || value === "yellow" || value === "green") {
    return value;
  }
  return "green";
}

function App() {
  const [fileState, setFileState] = useState<TrafficLightPayload>({
    light: "green",
    updatedAt: 0,
    blink: false,
  });
  const [now, setNow] = useState(Date.now());
  const [idleMs, setIdleMs] = useState(DEFAULT_IDLE_MS);
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPos, setMenuPos] = useState({ x: 0, y: 0 });

  useEffect(() => {
    const tick = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(tick);
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;

    const applyPayload = (payload: TrafficLightPayload) => {
      setFileState({
        light: normalizeLight(payload.light),
        updatedAt: payload.updatedAt ?? Date.now(),
        blink: payload.blink ?? payload.light === "yellow",
        label: payload.label,
      });
    };

    void (async () => {
      try {
        const initial = await invoke<TrafficLightPayload>("get_traffic_light_state");
        const settings = await invoke<AppSettings>("get_settings");
        if (!cancelled) {
          applyPayload(initial);
          setIdleMs(settings.idle_ms);
        }
      } catch {
        /* 浏览器预览时无 Tauri */
      }

      unlisten = await listen<TrafficLightPayload>(
        "traffic-light-changed",
        (event) => {
          if (!cancelled) applyPayload(event.payload);
        },
      );

      unlistenSettings = await listen<AppSettings>(
        "settings-changed",
        (event) => {
          if (!cancelled) setIdleMs(event.payload.idle_ms);
        },
      );
    })();

    return () => {
      cancelled = true;
      unlisten?.();
      unlistenSettings?.();
    };
  }, []);

  // 点击其他地方关闭菜单
  useEffect(() => {
    if (!menuVisible) return;
    const closeMenu = () => setMenuVisible(false);
    window.addEventListener("click", closeMenu);
    return () => window.removeEventListener("click", closeMenu);
  }, [menuVisible]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2) {
      // 右键
      e.preventDefault();
      setMenuPos({ x: e.clientX, y: e.clientY });
      setMenuVisible(true);
    } else if (e.button === 0) {
      // 左键拖动
      void getCurrentWindow().startDragging().catch(() => {});
    }
  }, []);

  const handleSettings = useCallback(() => {
    void invoke("show_settings_window");
    setMenuVisible(false);
  }, []);

  const handleExit = useCallback(() => {
    void getCurrentWindow().close().catch(() => {});
  }, []);

  const fileLight = normalizeLight(fileState.light);
  const lastAt = fileState.updatedAt ?? 0;
  const isWorking = fileLight === "yellow";
  const isIdle = isWorking && lastAt > 0 && now - lastAt >= idleMs;

  const displayLight: LightState = isIdle ? "red" : fileLight;
  const yellowBlink =
    !isIdle && fileLight === "yellow" && fileState.blink !== false;

  return (
    <div className="app-root">
      <div
        className="traffic-strip"
        onMouseDown={handleMouseDown}
        onContextMenu={(e) => e.preventDefault()}
        title="左键拖动，右键菜单"
      >
        {LIGHTS.map((light) => {
          const active = displayLight === light;
          const style = LIGHT_STYLE[light];
          const className = [
            "traffic-bulb",
            active && light === "yellow" && yellowBlink
              ? "traffic-bulb--blink"
              : "",
          ]
            .filter(Boolean)
            .join(" ");

          return (
            <div
              key={light}
              className={className}
              style={
                active && !(light === "yellow" && yellowBlink)
                  ? {
                      backgroundColor: style.on,
                      boxShadow: style.glow,
                    }
                  : active && light === "yellow" && yellowBlink
                    ? undefined
                    : {
                        backgroundColor: style.off,
                        boxShadow: "inset 0 2px 8px rgba(0, 0, 0, 0.4)",
                      }
              }
              aria-label={ARIA[light]}
            />
          );
        })}
      </div>

      {menuVisible && (
        <div
          className="context-menu"
          style={{ left: menuPos.x, top: menuPos.y }}
        >
          <div className="menu-item" onClick={handleSettings}>
            设置
          </div>
          <div className="menu-divider" />
          <div className="menu-item" onClick={handleExit}>
            退出
          </div>
        </div>
      )}
    </div>
  );
}

export default App;