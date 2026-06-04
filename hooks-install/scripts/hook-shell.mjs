#!/usr/bin/env node
import { readStdinJson, writeState } from "./state-file.mjs";

const input = await readStdinJson();
const command = String(input.command ?? "");

const needsConfirm =
  /\bcurl\b|\bwget\b|\bssh\b|\bscp\b|\brsync\b/i.test(command) ||
  /\bnpm\s+(install|publish|uninstall)\b/i.test(command) ||
  /\bpnpm\s+(add|install|publish)\b/i.test(command) ||
  /\byarn\s+(add|install)\b/i.test(command) ||
  /\bgit\s+push\b/i.test(command);

if (needsConfirm) {
  writeState("red", {
    blink: false,
    label: "待确认命令",
    detail: command.slice(0, 200),
  });
  console.log(
    JSON.stringify({
      permission: "ask",
      user_message: "该终端命令可能需要你确认，请查看后再继续。",
      agent_message: "Shell command flagged by traffic-light hook.",
    }),
  );
  process.exit(0);
}

writeState("yellow", { blink: true, label: "执行命令" });
console.log(JSON.stringify({ permission: "allow" }));
process.exit(0);
