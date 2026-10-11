import { invoke } from "@tauri-apps/api/core";
import "./App.css";

export default function ContextMenu() {
  const hide = () => {
    void invoke("hide_context_menu");
  };

  return (
    <div className="context-menu context-menu--window" onMouseLeave={hide}>
      <div
        className="menu-item"
        onClick={() => {
          void invoke("show_settings_window");
          hide();
        }}
      >
        设置
      </div>
      <div className="menu-divider" />
      <div
        className="menu-item"
        onClick={() => {
          void invoke("exit_app");
        }}
      >
        退出
      </div>
    </div>
  );
}
