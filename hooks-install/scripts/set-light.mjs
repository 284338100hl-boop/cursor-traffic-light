#!/usr/bin/env node
import { readStdinJson, writeState } from "./state-file.mjs";

const light = process.argv[2];
const mode = process.argv[3];
const input = await readStdinJson();

// 手动调用（node set-light.mjs red）时 input 为空，走普通写灯
const extra = {};

if (light === "yellow") {
  extra.blink = mode !== "steady";
  extra.label = mode === "steady" ? "待确认" : "思考中";
}

// --- Cursor hook 事件上下文（stdin 注入 JSON） ---
const event = input.hook_event_name ?? "";
const tool = input.tool ?? "";

// 主会话条目：记录 session_id，用于悬停显示对话名称
if (input.session_id) {
  extra._sessionId = input.session_id;
}

// 启动 subagent：记录到活跃任务列表
if (event === "PreToolUse" && tool === "Task") {
  const desc = input.tool_input?.description ?? input.tool_input?.prompt ?? "子任务";
  extra._addTask = {
    id: input.tool_call_id ?? `task-${Date.now()}`,
    kind: "subagent",
    name: String(desc).replace(/\s+/g, " ").slice(0, 40),
    type: input.tool_input?.subagent_type ?? "Task",
  };
}

// subagent 结束：按 tool_call_id 移出列表
if (event === "PostToolUse" && tool === "Task") {
  extra._removeTaskId = input.tool_call_id;
}

writeState(light, extra);
process.exit(0);