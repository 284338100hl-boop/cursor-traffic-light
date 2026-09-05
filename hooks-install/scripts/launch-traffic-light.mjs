#!/usr/bin/env node
import { execSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { readAppSetting } from "./state-file.mjs";

const EXE_NAME =
  process.platform === "win32"
    ? "cursor-agent-traffic-light.exe"
    : "cursor-agent-traffic-light";

const LOCK_PATH = path.join(
  os.homedir(),
  ".cursor",
  "agent-traffic-light.lock",
);

function isPidAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    if (process.platform === "win32") {
      const out = execSync(`tasklist /FI "PID eq ${pid}" /NH`, {
        encoding: "utf8",
      });
      return new RegExp(`\\b${pid}\\b`).test(out);
    }
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function isRunningByLock() {
  try {
    const pid = parseInt(fs.readFileSync(LOCK_PATH, "utf8").trim(), 10);
    if (isPidAlive(pid)) return true;
    fs.unlinkSync(LOCK_PATH);
  } catch {
    /* 无锁文件或进程已退出 */
  }
  return false;
}

function isRunningByName() {
  try {
    if (process.platform === "win32") {
      const out = execSync(`tasklist /FI "IMAGENAME eq ${EXE_NAME}" /NH`, {
        encoding: "utf8",
      });
      return new RegExp(EXE_NAME.replace(".", "\\."), "i").test(out);
    }
    execSync(`pgrep -x ${EXE_NAME}`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function isRunning() {
  return isRunningByLock() || isRunningByName();
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
