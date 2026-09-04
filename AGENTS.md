# AGENTS.md — DSH Launcher 项目指引

面向 AI 编码助手（以及协作者）的项目说明。

## 项目简介

Tauri 2 + Vue 3 的 macOS Apple Silicon 桌面启动器：管理多版本 DSH、
多实例（各自独立的 DSH_HOME、Profile、环境变量）。

**打开实例 = 用系统浏览器打开实例 URL（契约，勿回退）**：经 `open that`，
与 `open_external` 同策略（先 trim 再只放行 http 与 https）。
`open_instance_window` 保留命令名和只传 id 的签名，后端从 running 表查 URL；
**不存在任何实例 webview**。

- 主窗口 + 托盘：实例启停、版本安装、插件市场、后台任务、内嵌终端、设置。
- 运行数据在 Application Support 下的 `in.dsh-plug.dsh-launcher` 目录：
  `config.json`、`versions`、`homes`、`logs`。

## 常用命令

```bash
pnpm install              # 装依赖
pnpm dev                  # 浏览器预览（localStorage mock，没有 Rust 后端）
pnpm tauri dev            # 桌面联调（日常开发优先用这个验证）
pnpm build                # 等价于 vue-tsc --noEmit + vite build
pnpm test:release-notes   # 跑 ci 目录的 release-notes 测试
```

- 产物：`src-tauri/target/release/bundle/macos/dsh-launcher.app`（`.app` 包）；
  发版用的 dmg 在推送时才打，推到 GitHub release，日常开发不打。
- Rust 侧快速验证：在 `src-tauri/` 下 `cargo check` 或 `cargo build`。

## 重要：重新构建后必须替换自测用的 App

`pnpm tauri build --bundles app` 只产出 `target/release/bundle/macos/` 下的新版本，
**不会**自动更新自测用的 App。自测用的位置是 `/Applications/dsh-launcher.app`。
修改代码并重新打 `.app` 包后，必须执行以下步骤，否则自测跑的一直是旧版本：

```bash
# 1. 先退出运行中的自测副本
pkill -f "dsh-launcher.app" 2>/dev/null || true

# 2. 给产物 ad-hoc 深签名（bundler 不做 bundle 签名，跳过这步
#    codesign --verify 会报 "code has no resources but signature
#    indicates they must be present"）
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/dsh-launcher.app"

# 3. 替换 App（/Applications 当前用户可写，无需 sudo）
rm -rf /Applications/dsh-launcher.app
ditto "src-tauri/target/release/bundle/macos/dsh-launcher.app" /Applications/dsh-launcher.app

# 4. 校验签名（替换后必须通过）
codesign --verify --deep --strict /Applications/dsh-launcher.app
```

注意事项：

- 先确认没有正在运行的实例再删除，避免删除运行中的 App 导致行为异常。
- 替换前重新构建（`pnpm tauri build --bundles app`）保证产物是最新的。
- ad-hoc 签名仅用于本机自测；正式发版仍走各自的签名/公证流程。
