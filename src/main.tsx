import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Settings from "./Settings";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

async function init() {
  const label = await getCurrentWindow().label;

  root.render(
    <React.StrictMode>
      {label === "settings" ? <Settings /> : <App />}
    </React.StrictMode>,
  );
}

void init();
