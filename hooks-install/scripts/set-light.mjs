#!/usr/bin/env node
import { writeState } from "./state-file.mjs";

const light = process.argv[2];
const mode = process.argv[3];
const extra = {};

if (light === "yellow") {
  extra.blink = mode !== "steady";
  extra.label = mode === "steady" ? "待确认" : "思考中";
}

writeState(light, extra);
process.exit(0);
