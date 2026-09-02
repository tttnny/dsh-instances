<div align="center">

<img src="docs/banner.png" width="1024" height="512" alt="DSH Launcher">

# DSH Launcher

**多版本、多实例的 [DeepSeek Harness (DSH)](https://github.com/deepseek-ai/deepseek-harness) macOS 桌面启动器**（Tauri 2 + Vue 3，Apple Silicon）。

</div>

## 功能

- **多版本 / 多实例**：从 npm 安装多个 DSH 版本到隔离目录；同一版本可建多个实例，各自拥有独立名称、Profile 与环境变量（`DSH_HOME` 支持共用、自动收录 `~/.dsh` 或专属目录）。
- **一键启动**：选择实例 + Profile 启动，自动在独立窗口中打开 DSH Web GUI。
- **插件市场**：浏览 [dsh-plug.in](https://dsh-plug.in) 插件，按稳定版 / 测试版 / 最新提交三渠道安装到任意实例的 Profile，支持按 Profile 启用 / 禁用。
- **系统托盘**：双击打开最近使用的实例；菜单可停止运行中的实例；退出启动器时自动清理全部实例进程。
- **其他**：关闭窗口最小化到托盘、开机自启、中英双语界面。

## 开发

前置：Node ≥ 20、pnpm、Rust stable（Xcode Command Line Tools）、macOS 12+。

```bash
pnpm install        # 安装依赖
pnpm tauri dev      # 开发模式（前端 Vite + 后端 debug）
pnpm tauri build    # 打包（Apple Silicon .dmg）
pnpm dev            # 浏览器预览（localStorage mock）
```

## 运行数据

数据目录：`~/Library/Application Support/in.dsh-plug.dsh-launcher/`

- `config.json`：DSH_HOME / 版本 / 实例 / 设置
- `versions/<版本>/`：各版本隔离安装目录
- `homes/<实例名>/`：实例专属 DSH_HOME（如选择）
- `logs/`：启动器与实例运行日志

## 架构

- `src/`：Vue 3 前端（页面、store、API 封装、i18n）；浏览器环境走 localStorage mock。
- `src-tauri/src/`：Rust 后端——`config`（配置持久化）、`commands`（Tauri 命令）、`plugins`（插件市场与安装）、`tasks`（后台任务）、`process`（实例进程管理）、`terminal`（内置终端）、`tray`（托盘）、`windows`（窗口管理）。

## License

MIT
