#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const VALID = new Set(["red", "yellow", "green"]);

const DEFAULT_LABELS = {
  green: "已完成",
  yellow: "思考中",
  red: "已停顿",
};

// node:sqlite 在旧版 node 下可能不可用，动态加载失败时静默回退（无标题增强）
let sqlite = null;
try {
  sqlite = await import("node:sqlite");
} catch {
  sqlite = null;
}

// Cursor 全局状态库中对话标题的查询路径：
// %APPDATA%\Cursor\User\globalStorage\state.vscdb 的 composerHeaders 表，
// value 是 JSON，name 字段即 Cursor 界面显示的标题（自动生成或用户重命名）。
function getComposerDbPath() {
  const root =
    process.env.APPDATA?.trim() || path.join(os.homedir(), "AppData", "Roaming");
  return path.join(root, "Cursor", "User", "globalStorage", "state.vscdb");
}

// 按 sessionId 查询真实对话标题；查不到（活跃会话尚未落盘）或读取失败时返回 null
function queryComposerTitle(sessionId) {
  if (!sqlite || !sessionId) return null;
  try {
    const db = new sqlite.DatabaseSync(getComposerDbPath(), {
      readOnly: true,
      timeout: 2000,
    });
    try {
      const row = db
        .prepare("SELECT value FROM composerHeaders WHERE composerId = ?")
        .get(sessionId);
      if (row?.value) {
        const name = JSON.parse(row.value)?.name;
        if (typeof name === "string" && name.trim()) return name.trim();
      }
    } finally {
      db.close();
    }
  } catch {
    /* Cursor 正在占用数据库或表结构变化时忽略，走 transcript 回退 */
  }
  return null;
}

export function getStateFilePath() {
  const custom = process.env.AGENT_TRAFFIC_LIGHT_STATE?.trim();
  if (custom) return custom;
  return path.join(os.homedir(), ".cursor", "agent-traffic-light-state.json");
}

// Tauri store 路径，与桌面端 settings.json 一致
export function getAppStorePath() {
  if (process.platform === "win32") {
    const root =
      process.env.APPDATA?.trim() || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(root, "com.cursor.agent-traffic-light", "settings.json");
  }
  if (process.platform === "darwin") {
    return path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "com.cursor.agent-traffic-light",
      "settings.json",
    );
  }
  return path.join(
    os.homedir(),
    ".local",
    "share",
    "com.cursor.agent-traffic-light",
    "settings.json",
  );
}

export function readAppSetting(key, defaultValue) {
  try {
    const raw = fs.readFileSync(getAppStorePath(), "utf8");
    const store = JSON.parse(raw);
    const value = store[key];
    return value === undefined ? defaultValue : value;
  } catch {
    return defaultValue;
  }
}

export function readState() {
  const filePath = getStateFilePath();
  try {
    const raw = fs.readFileSync(filePath, "utf8");
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? {} : parsed;
  } catch {
    return {};
  }
}

// 从 Cursor 转录目录提取会话标题（best-effort）：
// 1) 优先查 Cursor 全局 state.vscdb 的 composerHeaders（真实标题，含用户重命名）
// 2) 回退：~/.cursor/projects/<project>/agent-transcripts/<sessionId>/<sessionId>.jsonl
//    第一行的 <user_query>（活跃会话尚未落盘标题时的近似值）
export function findSessionTitle(sessionId) {
  if (!sessionId) return null;
  const fromComposer = queryComposerTitle(sessionId);
  if (fromComposer) return fromComposer;
  const projectsRoot = path.join(os.homedir(), ".cursor", "projects");
  try {
    for (const project of fs.readdirSync(projectsRoot)) {
      const dir = path.join(projectsRoot, project, "agent-transcripts", sessionId);
      const file = path.join(dir, `${sessionId}.jsonl`);
      if (!fs.existsSync(file)) continue;
      const firstLine = fs.readFileSync(file, "utf8").split("\n")[0];
      if (!firstLine) continue;
      const parsed = JSON.parse(firstLine);
      const text = parsed?.message?.content?.[0]?.text ?? "";
      const m = text.match(/<user_query>([\s\S]*?)<\/user_query>/);
      const query = (m ? m[1] : text)
        .trim()
        .replace(/\s+/g, " ")
        .slice(0, 24);
      return query || null;
    }
  } catch {
    /* 目录或文件不存在时忽略 */
  }
  return null;
}

// 内部字段（下划线前缀）不写入状态文件，仅用于维护 tasks
function pickVisible(extra) {
  const visible = {};
  for (const [k, v] of Object.entries(extra)) {
    if (!k.startsWith("_")) visible[k] = v;
  }
  return visible;
}

// 维护"正在运行的任务"列表：
//   _sessionId    -> 设置/更新 main 会话条目
//   _addTask      -> 加入一个 subagent 条目
//   _removeTaskId -> 按 id 移除条目
function mergeTasks(prev, light, extra) {
  const tasks = Array.isArray(prev.tasks) ? [...prev.tasks] : [];

  if (extra._sessionId) {
    // 每次事件都重新提取标题：成本低（读一行），可自愈旧会话残留的错误标题
    const idx = tasks.findIndex((t) => t.kind === "main");
    const name = findSessionTitle(extra._sessionId) ?? "主对话";
    const mainTask = { id: extra._sessionId, kind: "main", name };
    if (idx >= 0) tasks[idx] = mainTask;
    else tasks.unshift(mainTask);
  }

  if (extra._addTask) {
    if (!tasks.some((t) => t.id === extra._addTask.id)) {
      tasks.push(extra._addTask);
    }
  }

  if (extra._removeTaskId) {
    const idx = tasks.findIndex((t) => t.id === extra._removeTaskId);
    if (idx >= 0) tasks.splice(idx, 1);
  }

  // 空闲/结束时只保留 main 会话
  if (light === "green") {
    for (let i = tasks.length - 1; i >= 0; i--) {
      if (tasks[i].kind !== "main") tasks.splice(i, 1);
    }
  }

  return tasks;
}

// 仅主对话真正结束时才接受绿灯（subagent 的 stop/sessionEnd 会误触）
function shouldAcceptGreen(tasks, extra) {
  if (tasks.some((t) => t.kind === "subagent")) {
    return false;
  }
  const mainTask = tasks.find((t) => t.kind === "main");
  if (extra._sessionId && mainTask?.id && extra._sessionId !== mainTask.id) {
    return false;
  }
  return true;
}

export function writeState(light, extra = {}) {
  if (!VALID.has(light)) return;
  const filePath = getStateFilePath();
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  const prev = readState();
  const tasks = mergeTasks(prev, light, extra);

  if (light === "green" && !shouldAcceptGreen(tasks, extra)) {
    return;
  }

  const payload = {
    light,
    updatedAt: Date.now(),
    label: DEFAULT_LABELS[light],
    source: "cursor-hook",
    ...pickVisible(extra),
    tasks,
  };
  if (light === "yellow" && payload.blink === undefined) {
    payload.blink = true;
  }
  fs.writeFileSync(filePath, `${JSON.stringify(payload)}\n`, "utf8");
}

export async function readStdinJson() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  const raw = Buffer.concat(chunks).toString("utf8").trim();
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch {
    return {};
  }
}