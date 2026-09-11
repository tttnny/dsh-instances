# AGENTS.md — DSH Launcher 项目指引

Tauri 2 + Vue 3 的 macOS Apple Silicon 桌面启动器：管理多版本 DSH 与多实例，
每个实例有独立的 DSH_HOME、Profile 和环境变量。主窗口 + 托盘提供实例启停、版本安装、
插件市场、后台任务、内嵌终端和设置；运行数据在 Application Support 下的
`in.dsh-plug.dsh-launcher` 目录（`config.json`、`versions`、`homes`、`logs`）。

**打开实例 = 用系统浏览器打开实例 URL（契约，勿回退）**：经 `open that`，
与 `open_external` 同策略（先 trim 再只放行 http 与 https）。
`open_instance_window` 保留命令名和只传 id 的签名，后端从 running 表查 URL；
**不存在任何实例 webview**。

## 常用命令

```bash
pnpm install              # 装依赖
pnpm dev                  # 浏览器预览（localStorage mock，没有 Rust 后端）
pnpm tauri dev            # 桌面联调（日常优先用这个验证）
pnpm build                # vue-tsc --noEmit + vite build
pnpm app:local            # 构建 .app、签名并安装到 /Applications 自测
pnpm test:release-notes   # 跑 ci 目录的 release-notes 测试
```

- 产物：`src-tauri/target/release/bundle/macos/dsh-launcher.app`。
- Rust 侧快速验证：在 `src-tauri/` 下 `cargo check` 或 `cargo build`。

## 本机自测 App

`pnpm tauri build --bundles app` 只产出 `target/release/bundle/macos/dsh-launcher.app`，
不会更新自测副本。自测副本固定在 `/Applications/dsh-launcher.app`，用脚本一步到位：

```bash
pnpm app:local              # 构建 + 签名 + 替换 + 校验
pnpm app:local --no-build   # 复用现有产物，只补签 + 替换
```

`ci/build-local-app.sh` 从钥匙串取 Apple Development 证书，经 `APPLE_SIGNING_IDENTITY`
交给 bundler 签名，然后退出运行中的实例、`ditto` 替换、`codesign --verify --deep --strict`
校验。签名身份是动态发现的，不硬编码。

**必须用证书签名，不能用 ad-hoc**：ad-hoc 的 designated requirement 就是 `cdhash`，
App 内容一变就失配，macOS 存下的「完全磁盘访问」等 TCC 授权随之失效——症状隐蔽，
系统设置里开关还开着、实际却没有权限。证书签名的 DR 只钉 bundle id 和证书，不含
cdhash，重建后不变。脚本一旦在 DR 里发现 cdhash 就直接报错退出。

注意：

- 别改 bundle id（`in.dsh-plug.dsh-launcher`）和安装路径，改了会重置授权。
- 证书约一年到期，DR 钉的是证书 CN（不是 Team ID），换发新证书需要重新授权一次。
- 这里只是本机自测签名；发版 dmg 由 CI 构建，未签名未公证，所以用户侧的授权不跨
  版本保留。要改需要 Developer ID 证书（付费）并配到 CI。

## 发版（CI 自动化）

`.github/workflows/ci.yml` 负责构建与发布，日常发版只推一个 tag：

```bash
git tag v<version> && git push origin v<version>   # tag 必须等于 package.json 的 version
```

- 推 tag → 质量门禁 → 打包 aarch64 dmg → 发正式 release（Latest）；收尾的
  `bump-version` job 把 manifest 抬到下一个 patch 并推回 main。
- 推 main → 同样流程，但发预发布 `v<version>-dev.<工作流序号>`，不动 Latest。
- 版本号以 `package.json` 为准，tag 必须与它相等，否则 `ci/resolve-release.sh` 直接报错。
  抬版本用 `node ci/bump-version.mjs v<已发布的tag>`，改完跑 `node ci/check-versions.mjs`
  校验（package.json / tauri.conf.json / Cargo.toml 三处一致）。
- Release notes 手写（中英双语）。流水线只在正文还是占位符时填入「下载表 + 提交列表」，
  已写好的 notes 重跑不会被覆盖。
- `cargo fmt --check` 是提示性的（`continue-on-error`），不阻塞发布：Rust 代码手工排版，
  全量 `cargo fmt` 会与并行 worktree 冲突。
- CI 挂掉时的手工补发：`pnpm tauri build --bundles dmg`，再
  `gh release create v<版本> <dmg路径> --title v<版本> --notes-file <notes>`。

推送前可在本地复现质量门禁：

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
node ci/check-versions.mjs
```
