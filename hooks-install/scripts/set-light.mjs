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
const event = String(input.hook_event_name ?? "");
const tool = String(input.tool_name ?? "");
extra._hookEvent = event;
if (input.generation_id) extra._generationId = input.generation_id;
if (input.status) extra._status = input.status;
// 子任务内部事件带着父 Task 的 tool_use_id，不能当成主会话
if (input.parent_tool_call_id) extra._parentToolCallId = input.parent_tool_call_id;

if (input.session_id) extra._sessionId = input.session_id;

const toolCallId = input.tool_use_id;
if (event.toLowerCase() === "pretooluse" && tool === "Task" && toolCallId) {
  const desc = input.tool_input?.description ?? input.tool_input?.prompt ?? "子任务";
  extra._addTask = {
    id: toolCallId,
    kind: "subagent",
    name: String(desc).replace(/\s+/g, " ").slice(0, 40),
    type: input.tool_input?.subagent_type ?? "Task",
  };
}

if (event.toLowerCase() === "posttooluse" && tool === "Task" && toolCallId) {
  extra._removeTaskId = toolCallId;
}

writeState(light, extra);
process.exit(0);