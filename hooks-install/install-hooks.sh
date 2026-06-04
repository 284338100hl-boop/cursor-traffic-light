#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST_DIR="${HOME}/.cursor/hooks/agent-traffic-light"
HOOKS_JSON="${HOME}/.cursor/hooks.json"
EXAMPLE="${SCRIPT_DIR}/hooks.user.json.example"

mkdir -p "${DEST_DIR}"
cp -f "${SCRIPT_DIR}/scripts/"* "${DEST_DIR}/"

echo "已复制脚本到: ${DEST_DIR}"
echo ""
echo "请将 hooks.user.json.example 中的 hooks 段合并进:"
echo "  ${HOOKS_JSON}"
echo ""
echo "示例文件:"
echo "  ${EXAMPLE}"
echo ""
echo "合并后重启 Cursor，并运行 npm run tauri dev 启动桌面灯。"
