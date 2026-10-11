import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Settings from "./Settings";
import ContextMenu from "./ContextMenu";
import LightToast from "./LightToast";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

async function init() {
  const label = await getCurrentWindow().label;
  const view =
    label === "settings" ? (
      <Settings />
    ) : label === "menu" ? (
      <ContextMenu />
    ) : label === "toast" ? (
      <LightToast />
    ) : (
      <App />
    );

  root.render(<React.StrictMode>{view}</React.StrictMode>);
}

void init();
