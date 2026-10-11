import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type ToastPayload = {
  light: "red" | "yellow" | "green";
  text: string;
  edge: "left" | "right" | "top" | "bottom";
};

const COLOR: Record<ToastPayload["light"], string> = {
  red: "#ef4444",
  yellow: "#f0c014",
  green: "#22c55e",
};

export default function LightToast() {
  const [payload, setPayload] = useState<ToastPayload | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<ToastPayload>("light-toast", (event) => {
      setPayload(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  if (!payload) return null;

  return (
    <button
      type="button"
      className={`light-toast light-toast--${payload.edge}`}
      onClick={() => {
        void invoke("expand_dock");
        void invoke("hide_light_toast");
      }}
    >
      <span
        className="light-toast-dot"
        style={{ backgroundColor: COLOR[payload.light] }}
      />
      <span className="light-toast-text">{payload.text}</span>
    </button>
  );
}
