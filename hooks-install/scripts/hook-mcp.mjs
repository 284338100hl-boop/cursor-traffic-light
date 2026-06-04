#!/usr/bin/env node
import { readStdinJson, writeState } from "./state-file.mjs";

const input = await readStdinJson();
const toolName = String(input.tool_name ?? input.toolName ?? "");

writeState("yellow", {
  blink: true,
  label: "调用工具",
  detail: toolName.slice(0, 120),
});
console.log(JSON.stringify({ permission: "allow" }));
process.exit(0);
