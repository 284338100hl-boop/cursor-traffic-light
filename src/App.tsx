import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import "./App.css";

type LightState = "red" | "yellow" | "green";

type ActivityTask = {
  id: string;
  kind: "main" | "subagent" | string;
  name: string;
  type?: string;
};

type TrafficLightPayload = {
  light: string;
  updatedAt?: number;
  label?: string;
  blink?: boolean;
  tasks?: ActivityTask[];
};

type AppSettings = {
  idle_ms: number;
  notify_on_complete: boolean;
  always_on_top: boolean;
  show_hover_tooltip: boolean;
  auto_launch_with_cursor: boolean;
};

const DEFAULT_IDLE_MS = 30_000;

// 窗口尺寸：悬停时从窄条扩展出 tooltip 区域
const MAIN_W = 64;
const MAIN_H = 204;
const TOOLTIP_W = 320;
const MENU_MIN_W = 108;

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
  const [showHoverTooltip, setShowHoverTooltip] = useState(true);
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPos, setMenuPos] = useState({ x: 0, y: 0 });
  const [tooltipVisible, setTooltipVisible] = useState(false);

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
        tasks: payload.tasks,
      });
    };

    void (async () => {
      try {
        const initial = await invoke<TrafficLightPayload>("get_traffic_light_state");
        const settings = await invoke<AppSettings>("get_settings");
        if (!cancelled) {
          applyPayload(initial);
          setIdleMs(settings.idle_ms);
          setShowHoverTooltip(settings.show_hover_tooltip);
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
          if (!cancelled) {
            setIdleMs(event.payload.idle_ms);
            setShowHoverTooltip(event.payload.show_hover_tooltip);
          }
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

  const restoreWindowWidth = useCallback(() => {
    const w = tooltipVisible && showHoverTooltip ? TOOLTIP_W : MAIN_W;
    void getCurrentWindow()
      .setSize(new LogicalSize(w, MAIN_H))
      .catch(() => {});
  }, [tooltipVisible, showHoverTooltip]);

  useEffect(() => {
    if (menuVisible) return;
    restoreWindowWidth();
  }, [menuVisible, restoreWindowWidth]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2) {
      e.preventDefault();
      const x = e.clientX;
      const y = e.clientY;
      const menuW = Math.max(MAIN_W, x + MENU_MIN_W + 8);
      void getCurrentWindow()
        .setSize(new LogicalSize(menuW, MAIN_H))
        .catch(() => {})
        .finally(() => {
          setMenuPos({ x, y });
          setMenuVisible(true);
        });
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
  // 有 subagent 在运行说明主 agent 在等待子任务，不算停顿
  const hasActiveSubagent = (fileState.tasks ?? []).some(
    (t) => t.kind === "subagent",
  );
  const isIdle =
    isWorking && !hasActiveSubagent && lastAt > 0 && now - lastAt >= idleMs;

  const displayLight: LightState = isIdle ? "red" : fileLight;
  const yellowBlink =
    !isIdle && fileLight === "yellow" && fileState.blink !== false;

  // 悬停内容：只显示主对话标题（单行）；空闲显示"暂无任务"
  const currentTitle =
    fileLight === "green"
      ? null
      : (fileState.tasks ?? []).find((t) => t.kind === "main")?.name ?? null;

  // 悬停时扩宽窗口显示任务列表，移开后恢复
  const handleMouseEnter = useCallback(() => {
    if (!showHoverTooltip) return;
    setTooltipVisible(true);
    void getCurrentWindow()
      .setSize(new LogicalSize(TOOLTIP_W, MAIN_H))
      .catch(() => {});
  }, [showHoverTooltip]);

  const handleMouseLeave = useCallback(() => {
    if (!showHoverTooltip) return;
    setTooltipVisible(false);
    if (menuVisible) return;
    void getCurrentWindow()
      .setSize(new LogicalSize(MAIN_W, MAIN_H))
      .catch(() => {});
  }, [showHoverTooltip, menuVisible]);

  useEffect(() => {
    if (showHoverTooltip || menuVisible) return;
    setTooltipVisible(false);
    void getCurrentWindow()
      .setSize(new LogicalSize(MAIN_W, MAIN_H))
      .catch(() => {});
  }, [showHoverTooltip, menuVisible]);

  return (
    <div className="app-root">
      <div
        className="traffic-strip"
        onMouseDown={handleMouseDown}
        onContextMenu={(e) => e.preventDefault()}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
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

      {tooltipVisible && showHoverTooltip && (
        <div className="tooltip">
          <span className="tooltip-name">
            {currentTitle ?? "暂无任务"}
          </span>
        </div>
      )}

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