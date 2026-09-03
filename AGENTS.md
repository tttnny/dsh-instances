# AGENTS.md - DSH Launcher

Tauri 2 + Vue 3 macOS Apple Silicon launcher: multi-version DSH, multi-instance with own DSH_HOME, Profile, env.

## Commands

- pnpm install; pnpm dev for browser preview with localStorage mock; pnpm tauri dev for desktop.
- pnpm build runs vue-tsc noEmit plus vite build. Rust quick check via cargo check or cargo build.
- Packaging is explicit only: pnpm tauri build makes dmg under src-tauri target release bundle, copy it to Downloads for manual install. Daily dev never packages.
- pnpm test:release-notes runs ci release-notes test.

## Layout

- src is Vue 3 frontend: views, stores, api index, i18n. api index branches on isTauri: invoke on desktop, mock in browser.
- src-tauri src dirs: config, commands, process, tray, windows for main window only, terminal, plugins, tasks.
- Runtime data lives under Application Support in.dsh-plug.dsh-launcher: config.json, versions, homes, logs.

## Conventions

- Opening an instance means opening its URL in the system browser via open that, same policy as open_external: trim then allow http https only.
- open_instance_window keeps its name and id-only signature; backend resolves URL from running table. No instance webviews exist.
- i18n changes values not keys; keep the copy-URL buttons.
- Backend user-facing errors stay Chinese; preview mock must fail the same way desktop does instead of silent no-op.
- Parallel work uses worktrees dir per topic, untracked, merge back with zero-conflict check before commit.

## Gotchas

- Unrelated dirty state exists: .github deletions and worktrees dir. Stage only target files on commit.
- Preview window.open must stay in the click sync chain or popup blockers eat it.
- Headless cannot do real desktop clicks: after static checks pass, list manual steps for a human to run under tauri dev.

