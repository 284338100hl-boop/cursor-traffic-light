# Cursor Agent Traffic Light

桌面竖条红绿灯，显示 Cursor Agent 当前状态。通过 Cursor Hooks 写入状态文件，Tauri 小窗实时监听并更新 UI。

## 灯色含义

| 灯色 | 含义 |
|------|------|
| 绿 | 空闲 / 已完成（`stop`、`sessionEnd` 等），常亮 |
| 黄 | 思考、回答、执行工具或命令，慢闪（约 3.2 秒一轮） |
| 红 | 命中需确认的 Shell 命令（`hook-shell`），或黄灯超过 30 秒无更新（桌面端判断为「已停顿」） |

停顿阈值可在 `src/App.tsx` 中修改 `IDLE_MS`（默认 `30000`）。

## 工作原理

```
Cursor Hooks → ~/.cursor/agent-traffic-light-state.json → Tauri 监听 → 桌面小窗
```

- Hooks 脚本：`hooks-install/scripts/`
- 状态文件默认路径：`%USERPROFILE%\.cursor\agent-traffic-light-state.json`
- 自定义路径：设置环境变量 `AGENT_TRAFFIC_LIGHT_STATE`

## 环境要求

- [Node.js](https://nodejs.org/)（建议 LTS）
- [Rust](https://www.rust-lang.org/)（`stable`）
- Windows：Visual Studio **C++ 构建工具**（Tauri 前置依赖）
- Windows： [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)（Win10/11 通常已自带）

国内 Rust / cargo 镜像配置见仓库内 `SETUP.txt`。

## 快速开始

### 1. 安装依赖并开发运行

```bash
npm install
npm run tauri dev
```

### 2. 安装 Cursor Hooks（用户级）

PowerShell：

```powershell
.\hooks-install\install-hooks.ps1
```

Git Bash：

```bash
bash hooks-install/install-hooks.sh
```

将 `hooks-install/hooks.json` 中的 `hooks` 段合并进：

```
%USERPROFILE%\.cursor\hooks.json
```

脚本需位于：

```
%USERPROFILE%\.cursor\hooks\agent-traffic-light\*.mjs
```

合并后**重启 Cursor**，在设置 → Hooks 中确认已加载。

### 3. 打包免安装 exe

```bash
npm run build:exe
```

产物：

```
src-tauri/target/release/cursor-agent-traffic-light.exe
```

双击即可运行，无需安装程序。

## 手动测灯

不启动 Cursor 也可验证小窗：

```bash
node hooks-install/scripts/set-light.mjs red
node hooks-install/scripts/set-light.mjs yellow
node hooks-install/scripts/set-light.mjs green
```

## 项目结构

```
├── src/                 # React 前端（三灯 UI）
├── src-tauri/           # Tauri / Rust（读状态、文件监听）
├── hooks-install/       # Cursor Hooks 与安装脚本
├── SETUP.txt            # 详细安装说明（含国内镜像）
└── package.json
```

## 常见问题

**移动项目目录后 `npm run build:exe` 报路径找不到**

`src-tauri/target` 中缓存了旧绝对路径，清理后重编：

```bash
cd src-tauri && cargo clean && cd .. && npm run build:exe
```

**小窗无变化**

1. 确认 `~/.cursor/hooks.json` 路径与配置正确  
2. 确认状态文件有更新（可手动执行 `set-light.mjs`）  
3. 桌面小窗需保持运行

## 技术栈

- 前端：React 19、TypeScript、Vite 7  
- 桌面：Tauri 2（透明无边框、置顶、固定尺寸 64×204）

## License

未指定许可证时，请按仓库所有者约定使用。
