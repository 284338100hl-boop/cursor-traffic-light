import { useEffect, useState } from "react";
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

const IDLE_MS = 30_000;

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

function startDrag(e: React.MouseEvent) {
  if (e.button !== 0) return;
  void getCurrentWindow().startDragging().catch(() => {});
}

function App() {
  const [fileState, setFileState] = useState<TrafficLightPayload>({
    light: "green",
    updatedAt: 0,
    blink: false,
  });
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const tick = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(tick);
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

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
        const initial =
          await invoke<TrafficLightPayload>("get_traffic_light_state");
        if (!cancelled) applyPayload(initial);
      } catch {
        /* 浏览器预览时无 Tauri */
      }

      unlisten = await listen<TrafficLightPayload>(
        "traffic-light-changed",
        (event) => {
          if (!cancelled) applyPayload(event.payload);
        },
      );
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const fileLight = normalizeLight(fileState.light);
  const lastAt = fileState.updatedAt ?? 0;
  const isWorking = fileLight === "yellow";
  const isIdle = isWorking && lastAt > 0 && now - lastAt >= IDLE_MS;

  const displayLight: LightState = isIdle ? "red" : fileLight;
  const yellowBlink =
    !isIdle && fileLight === "yellow" && fileState.blink !== false;

  return (
    <div className="app-root">
      <div
        className="traffic-strip"
        onMouseDown={startDrag}
        title="拖动移动窗口"
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
    </div>
  );
}

export default App;
