import { useEffect, useState, useCallback, useRef } from "react";
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
  /** hooks 在主会话真正结束时置 true；工具轮次误闪绿灯时为 false */
  notifyComplete?: boolean;
};

type AppSettings = {
  idle_ms: number;
  notify_on_complete: boolean;
  notify_ntfy: boolean;
  ntfy_topic: string;
  always_on_top: boolean;
  show_hover_tooltip: boolean;
  auto_launch_with_cursor: boolean;
};

const DEFAULT_IDLE_MS = 30_000;

const MAIN_W = 140;
const MAIN_H = 36;
const TOOLTIP_EXTRA = 260;

const LIGHTS: LightState[] = ["red", "yellow", "green"];

const LIGHT_STYLE: Record<
  LightState,
  { on: string; off: string; glow: string }
> = {
  red: {
    on: "#ef4444",
    off: "#4d2226",
    glow: "0 0 22px rgba(239, 68, 68, 0.55), inset 0 0 10px rgba(255, 255, 255, 0.16)",
  },
  yellow: {
    on: "#f0c014",
    off: "#3d3518",
    glow: "0 0 22px rgba(240, 192, 20, 0.75), inset 0 0 10px rgba(255, 255, 255, 0.18)",
  },
  green: {
    on: "#22c55e",
    off: "#1c3c32",
    glow: "0 0 22px rgba(34, 197, 94, 0.55), inset 0 0 10px rgba(255, 255, 255, 0.16)",
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
  const [tooltipVisible, setTooltipVisible] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const expanding = useRef(false);
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const userDragging = useRef(false);
  const prevDisplay = useRef<LightState | null>(null);

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
        notifyComplete: payload.notifyComplete === true,
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

  useEffect(() => {
    let cancelled = false;
    let unResized: (() => void) | undefined;
    let unMoved: (() => void) | undefined;
    let unDock: (() => void) | undefined;
    let moveTimer: number | undefined;
    void (async () => {
      const win = getCurrentWindow();
      const moved = await win.onMoved(() => {
        if (!userDragging.current) return;
        window.clearTimeout(moveTimer);
        moveTimer = window.setTimeout(() => {
          userDragging.current = false;
          void invoke("dock_drag_end");
        }, 280);
      });
      unDock = await listen<{ collapsed: boolean }>("dock-changed", (event) => {
        setCollapsed(event.payload.collapsed);
      });
      if (cancelled) {
        moved();
        unDock?.();
        return;
      }
      unMoved = moved;
    })();
    return () => {
      cancelled = true;
      window.clearTimeout(moveTimer);
      unResized?.();
      unMoved?.();
      unDock?.();
    };
  }, []);

  const applyBaseSize = useCallback(() => {
    if (collapsed) return;
    expanding.current = false;
    void getCurrentWindow()
      .setSize(new LogicalSize(MAIN_W, MAIN_H))
      .catch(() => {});
  }, [collapsed]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2) {
      e.preventDefault();
      void invoke("show_context_menu", {
        clientX: e.clientX,
        clientY: e.clientY,
      });
    } else if (e.button === 0) {
      dragStart.current = { x: e.clientX, y: e.clientY };
      userDragging.current = true;
      void getCurrentWindow().startDragging().catch(() => {});
    }
  }, []);

  const handleClick = useCallback((e: React.MouseEvent) => {
    if (!collapsed || e.button !== 0) return;
    const start = dragStart.current;
    dragStart.current = null;
    if (!start) return;
    const dist = Math.hypot(e.clientX - start.x, e.clientY - start.y);
    if (dist < 5) {
      void invoke("expand_dock");
    }
  }, [collapsed]);

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

  // 悬停跟灯色解耦：看 tasks / 会话是否仍在进行。
  // 误闪绿灯时 notifyComplete 为 false，只要还有任务就显示标题，不显示「暂无任务」。
  const activityTasks = fileState.tasks ?? [];
  const currentTitle =
    fileState.notifyComplete || activityTasks.length === 0
      ? null
      : activityTasks.find((t) => t.kind === "main")?.name ??
        activityTasks.find((t) => t.kind === "subagent")?.name ??
        null;

  const handleMouseEnter = useCallback(() => {
    if (!showHoverTooltip || collapsed) return;
    setTooltipVisible(true);
    expanding.current = true;
    void getCurrentWindow()
      .setSize(new LogicalSize(MAIN_W + TOOLTIP_EXTRA, MAIN_H))
      .catch(() => {});
  }, [showHoverTooltip, collapsed]);

  const handleMouseLeave = useCallback(() => {
    if (!showHoverTooltip || collapsed) return;
    setTooltipVisible(false);
    applyBaseSize();
  }, [showHoverTooltip, collapsed, applyBaseSize]);

  useEffect(() => {
    if (showHoverTooltip) return;
    setTooltipVisible(false);
    applyBaseSize();
  }, [showHoverTooltip, applyBaseSize]);

  useEffect(() => {
    if (tooltipVisible) return;
    applyBaseSize();
  }, [tooltipVisible, applyBaseSize]);

  useEffect(() => {
    if (prevDisplay.current === null) {
      prevDisplay.current = displayLight;
      return;
    }
    // 黄灯是发出问题后立刻写入的，不提示；只在停顿转红、任务完成转绿时提示
    if (
      prevDisplay.current !== displayLight &&
      collapsed &&
      displayLight !== "yellow"
    ) {
      void invoke("show_light_toast", { light: displayLight });
    }
    prevDisplay.current = displayLight;
  }, [displayLight, collapsed]);

  return (
    <div className={`app-root${collapsed ? " collapsed" : ""}`}>
      <div
        className="traffic-strip"
        onMouseDown={handleMouseDown}
        onClick={handleClick}
        onContextMenu={(e) => e.preventDefault()}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {collapsed ? (
          <div
            className="peek-lamp"
            style={{
              backgroundColor: LIGHT_STYLE[displayLight].on,
              boxShadow: LIGHT_STYLE[displayLight].glow,
            }}
          />
        ) : (
          LIGHTS.map((light) => {
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
          })
        )}
      </div>

      {tooltipVisible && showHoverTooltip && !collapsed && (
        <div className="tooltip">
          <span className="tooltip-name">
            {currentTitle ?? "暂无任务"}
          </span>
        </div>
      )}
    </div>
  );
}

export default App;