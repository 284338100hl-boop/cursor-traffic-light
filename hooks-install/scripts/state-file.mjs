import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const VALID = new Set(["red", "yellow", "green"]);

const DEFAULT_LABELS = {
  green: "已完成",
  yellow: "思考中",
  red: "已停顿",
};

export function getStateFilePath() {
  const custom = process.env.AGENT_TRAFFIC_LIGHT_STATE?.trim();
  if (custom) return custom;
  return path.join(os.homedir(), ".cursor", "agent-traffic-light-state.json");
}

export function writeState(light, extra = {}) {
  if (!VALID.has(light)) return;
  const filePath = getStateFilePath();
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const payload = {
    light,
    updatedAt: Date.now(),
    label: DEFAULT_LABELS[light],
    source: "cursor-hook",
    ...extra,
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
