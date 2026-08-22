#!/usr/bin/env node
import { execSync, spawn } from "node:child_process";
import { readAppSetting } from "./state-file.mjs";

const EXE_NAME =
  process.platform === "win32"
    ? "cursor-agent-traffic-light.exe"
    : "cursor-agent-traffic-light";

function isRunning() {
  try {
    if (process.platform === "win32") {
      const out = execSync(`tasklist /FI "IMAGENAME eq ${EXE_NAME}" /NH`, {
        encoding: "utf8",
      });
      return out.toLowerCase().includes(EXE_NAME.toLowerCase());
    }
    execSync(`pgrep -x ${EXE_NAME}`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

const enabled = readAppSetting("auto_launch_with_cursor", false);
if (!enabled || isRunning()) {
  process.exit(0);
}

const exePath = readAppSetting("exe_path", "");
if (!exePath || typeof exePath !== "string") {
  process.exit(0);
}

try {
  spawn(exePath, [], { detached: true, stdio: "ignore", windowsHide: true }).unref();
} catch {
  /* 路径无效或权限不足时静默失败 */
}

process.exit(0);
