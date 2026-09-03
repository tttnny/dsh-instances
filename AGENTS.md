# AGENTS.md - DSH Launcher

Tauri 2 + Vue 3 的 macOS Apple Silicon 桌面启动器：管理多版本 DSH、多实例（各自独立的 DSH_HOME、Profile、环境变量）。

## 命令

- pnpm install 装依赖；pnpm dev 跑浏览器预览（localStorage mock，没有 Rust 后端）；pnpm tauri dev 跑桌面联调。
- pnpm build 等价于 vue-tsc noEmit 加 vite build；Rust 侧用 cargo check 或 cargo build 快速验证。
- 打包分两种：agent 自测可自行 pnpm tauri build --bundles app 产出 .app（在 src-tauri target release bundle macos 下）自己安装验证（绝不覆盖本机 /Applications，用临时目录或直接运行），需要交付给人时拷到 Downloads 目录覆盖；发版用的 dmg 只在被明确要求时打，推到 GitHub release。日常开发优先 dev 验证。
- pnpm test:release-notes 跑 ci 目录的 release-notes 测试。

## 目录

- src 是 Vue 3 前端：views、stores、api index、i18n。api index 顶部按 isTauri 分叉：桌面走 invoke，浏览器预览走 localStorage mock。
- src-tauri src 下：config 管配置持久化，commands 是 Tauri 命令，process 管实例进程，tray 管托盘，windows 只剩主窗口逻辑，terminal 是内嵌终端，plugins 和 tasks 管市场与后台任务。
- 运行数据在 Application Support 下的 in.dsh-plug.dsh-launcher 目录：config.json、versions、homes、logs。

## 约定

- 打开实例等于用系统浏览器打开实例 URL（经 open that，与 open_external 同策略：先 trim 再只放行 http 与 https）。
- open_instance_window 保留命令名和只传 id 的签名，后端从 running 表查 URL；不存在任何实例 webview。
- i18n 只改文案值不改 key；复制链接按钮一律保留。
- 后端面向用户的报错文案用中文；预览 mock 必须和桌面端一致地抛错，不许静默 no-op。
- 并行开发时每路需求开独立 worktrees 子目录隔离（untracked、不提交），合回主工作区并做零冲突校验后再提交。

## 坑位

- 工作区现有与需求无关的脏状态：.github 目录删除与 worktrees 目录。提交时只暂存目标文件，不许顺手带上。
- 预览环境的 window.open 必须留在点击同步调用链里，否则会被弹窗拦截。
- headless 环境点不了真实桌面交互：静态检查（check、vue-tsc、dev 返回 200）通过后，列出人工补点清单请人用 tauri dev 跑一遍。

