// Profile plugins: per-instance/profile enable / disable / uninstall plumbing
// over the profile manifest (package.json) and cordis.patch.yml, driven
// through the instance's own `dsh plugin` CLI.

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Public OAuth App client id used to boost unauthenticated GitHub API quota
/// from 60 to 5000 requests/hour (an anonymous client-id parameter, no
/// authorization or token storage required). App: "DSH Launcher".
const GITHUB_CLIENT_ID: &str = "Ov23li6vtlVd83282YL6";

/// Build a GitHub API URL with the anonymous client-id quota boost.
/// `pub(crate)` so `update.rs` (launcher self-update check) can reuse the
/// same quota-boosted endpoint instead of the rate-limited `releases.atom`.
pub(crate) fn github_api_url(path: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("https://api.github.com{path}{sep}client_id={GITHUB_CLIENT_ID}")
}

// ---------------------------------------------------------------------------
// Installed plugin (per instance/profile)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct InstalledPlugin {
    /// Package name / id (e.g. "@dsh-plugin/dsh-auxiliary").
    pub id: String,
    /// Installed version spec as recorded in the profile manifest.
    pub version: Option<String>,
    /// Whether the plugin is currently enabled (not disabled in cordis.patch.yml).
    pub enabled: bool,
    /// The cordis plugin id used in cordis.patch.yml (disables/insert rows).
    pub cordis_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPluginsEnabledInput {
    #[serde(alias = "home_id")]
    pub home_id: String,
    pub profile: String,
    #[serde(alias = "plugin_ids")]
    pub plugin_ids: Vec<String>,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

fn http_client() -> Result<reqwest::Client, String> {
    crate::proxy::apply(reqwest::Client::builder())
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("dsh-launcher")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// Fetch and parse a JSON document with a size cap.
async fn fetch_json(url: &str, cap: usize) -> Result<serde_json::Value, String> {
    let client = http_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败 {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("请求失败 {url}: HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败 {url}: {e}"))?;
    if bytes.len() > cap {
        return Err(format!("响应过大 {url}"));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("解析 JSON 失败 {url}: {e}"))
}

/// `pub(crate)` so `commands.rs` (GitHub release tag listing) can reuse the
/// same HTTP client and size cap.
pub(crate) async fn fetch_json_pub(url: &str, cap: usize) -> Result<serde_json::Value, String> {
    fetch_json(url, cap).await
}

// ---------------------------------------------------------------------------
// Profile manifest helpers (read/write package.json + cordis.patch.yml)
// ---------------------------------------------------------------------------

/// Path of a profile dir under a DSH_HOME.
fn profile_dir(home_path: &std::path::Path, profile: &str) -> std::path::PathBuf {
    home_path.join("profiles").join(profile)
}

/// Read the profile package.json (dsh.profile.bundles + dependencies).
fn read_profile_manifest(dir: &std::path::Path) -> Result<serde_json::Value, String> {
    let path = dir.join("package.json");
    if !path.exists() {
        return Ok(serde_json::json!({
            "private": true,
            "dependencies": {},
            "dsh": { "profile": { "bundles": [] } },
        }));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读取 package.json 失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 package.json 失败: {e}"))
}

/// cordis id for a package: bundles register under their unscoped short name
/// (dsh-auxiliary) unless the package declares otherwise. We default to the
/// last path segment without the scope.
pub fn cordis_id_of(package: &str) -> String {
    let last = package.rsplit('/').next().unwrap_or(package);
    last.to_string()
}

// ---------------------------------------------------------------------------
// Commands: installed plugin listing (per instance + profile)
// ---------------------------------------------------------------------------

/// Lists plugins installed into a HOME's profile, excluding core
/// @deepseek-ai/* packages. Reads the profile manifest (dependencies +
/// bundles) and cordis.patch.yml (disabled rows).
#[tauri::command(rename_all = "snake_case")]
pub async fn list_installed_plugins(
    state: State<'_, AppState>,
    home_id: String,
    profile: String,
) -> Result<Vec<InstalledPlugin>, String> {
    let (home_path, _) = resolve_home_paths(&state, &home_id)?;
    let dir = profile_dir(&home_path, &profile);
    let manifest = read_profile_manifest(&dir)?;

    let mut ids: Vec<String> = Vec::new();
    let mut versions: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if let Some(deps) = manifest.get("dependencies").and_then(|d| d.as_object()) {
        for (name, spec) in deps {
            if name.starts_with("@deepseek-ai/") {
                continue;
            }
            ids.push(name.clone());
            versions.insert(name.clone(), spec.as_str().unwrap_or("").to_string());
        }
    }
    if let Some(bundles) = manifest
        .pointer("/dsh/profile/bundles")
        .and_then(|b| b.as_array())
    {
        for b in bundles {
            if let Some(name) = b.as_str() {
                if name.starts_with("@deepseek-ai/") || ids.iter().any(|i| i == name) {
                    continue;
                }
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    ids.dedup();

    // Disabled set from cordis.patch.yml (`- id: <cordis-id>` + `disabled: true`).
    let disabled = read_disabled_ids(&dir);

    let out = ids
        .into_iter()
        .map(|id| {
            let cordis_id = cordis_id_of(&id);
            let enabled = !disabled.contains(&cordis_id) && !disabled.contains(&id);
            InstalledPlugin {
                version: versions.get(&id).cloned(),
                enabled,
                cordis_id: Some(cordis_id),
                id,
            }
        })
        .collect();
    Ok(out)
}

/// Parse disabled cordis ids from a profile's cordis.patch.yml. We do a
/// lightweight line scan (avoid pulling a YAML parser dependency for this).
fn read_disabled_ids(dir: &std::path::Path) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let path = dir.join("cordis.patch.yml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return set;
    };
    let mut current_id: Option<String> = None;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("- id:") {
            current_id = Some(t.trim_start_matches("- id:").trim().to_string());
        } else if t.starts_with("id:") && !line.starts_with(' ') && !line.starts_with('\t') {
            current_id = Some(t.trim_start_matches("id:").trim().to_string());
        } else if t == "disabled: true" {
            if let Some(id) = current_id.take() {
                set.insert(id);
            }
        } else if t.starts_with("- ") && !t.starts_with("- id:") {
            current_id = None;
        }
    }
    set
}

/// Resolve a HOME to (home_path, version_dir): plugin file edits need the
/// HOME path; `dsh plugin remove` runs through a CLI binary, so any
/// installed version serves (the newest installed one wins).
pub(crate) fn resolve_home_paths(
    state: &State<'_, AppState>,
    home_id: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let cfg = state.config.lock().unwrap();
    let home = cfg
        .homes
        .iter()
        .find(|h| h.id == home_id)
        .ok_or_else(|| "DSH_HOME 不存在".to_string())?;
    let version = cfg.versions.last().ok_or_else(|| "尚未安装任何 DSH 版本".to_string())?;
    Ok((home.path.clone(), version.dir.clone()))
}

// ---------------------------------------------------------------------------
// Commands: enable / disable (cordis.patch.yml disabled rows)
// ---------------------------------------------------------------------------

/// Sets plugins enabled/disabled in a profile's cordis.patch.yml by adding or
/// removing `disabled: true` rows. Batch-capable via plugin_ids.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_plugins_enabled(
    state: State<'_, AppState>,
    input: SetPluginsEnabledInput,
) -> Result<(), String> {
    let (home_path, _) = resolve_home_paths(&state, &input.home_id)?;
    let dir = profile_dir(&home_path, &input.profile);
    let patch_path = dir.join("cordis.patch.yml");

    let mut raw = if patch_path.exists() {
        std::fs::read_to_string(&patch_path)
            .map_err(|e| format!("读取 cordis.patch.yml 失败: {e}"))?
    } else {
        String::new()
    };

    for package in &input.plugin_ids {
        let cordis_id = cordis_id_of(package);
        raw = set_disabled_row(&raw, &cordis_id, input.enabled);
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 profile 目录失败: {e}"))?;
    std::fs::write(&patch_path, raw).map_err(|e| format!("写入 cordis.patch.yml 失败: {e}"))?;
    Ok(())
}

/// Input for uninstalling a plugin from a profile.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPluginInput {
    #[serde(alias = "home_id")]
    pub home_id: String,
    pub profile: String,
    #[serde(alias = "plugin_id")]
    pub plugin_id: String,
}

/// Uninstalls a plugin from a HOME's profile through
/// `dsh plugin --profile <name> remove <id>` (the CLI removes the dependency
/// and reconciles dsh.profile.bundles), then drops the plugin's
/// cordis.patch.yml rows (insert / disabled), which the CLI does not manage.
#[tauri::command(rename_all = "snake_case")]
pub async fn uninstall_plugin(
    state: State<'_, AppState>,
    input: UninstallPluginInput,
) -> Result<(), String> {
    let (home_path, version_dir) = resolve_home_paths(&state, &input.home_id)?;
    let dir = profile_dir(&home_path, &input.profile);
    if !dir.exists() {
        return Err(format!("Profile「{}」不存在", input.profile));
    }

    // `dsh plugin remove <id>` through an installed CLI: it removes the
    // dependency and reconciles dsh.profile.bundles (a name that is no
    // longer an installed bundle leaves the layer stack), so the manifest is
    // never edited by hand here. Runs synchronously; the frontend shows its
    // own progress state.
    run_dsh_plugin_sync(
        &state,
        &PluginCliTarget {
            version_dir: &version_dir,
            home_path: &home_path,
            profile: &input.profile,
        },
        &PluginCliOp {
            subcommand: "remove",
            spec: &input.plugin_id,
            loglevel: "warn",
        },
    )?;

    // 2. Drop the plugin's rows from cordis.patch.yml (insert rows mount the
    //    plugin; disabled rows gate it). Reuse the block-stripping logic in
    //    set_disabled_row by removing any block whose id matches.
    let patch_path = dir.join("cordis.patch.yml");
    if patch_path.exists() {
        let raw = std::fs::read_to_string(&patch_path)
            .map_err(|e| format!("读取 cordis.patch.yml 失败: {e}"))?;
        let cordis_id = cordis_id_of(&input.plugin_id);
        let cleaned = strip_cordis_rows(&raw, &cordis_id, &input.plugin_id);
        if cleaned != raw {
            std::fs::write(&patch_path, &cleaned)
                .map_err(|e| format!("写入 cordis.patch.yml 失败: {e}"))?;
        }
    }

    Ok(())
}

/// Strips every cordis.patch.yml block whose id equals `cordis_id` (matching
/// plain `- id:` / `id:` rows, including `- insert:` wrappers) and restores
/// the `[]` placeholder when the document becomes empty.
fn strip_cordis_rows(raw: &str, cordis_id: &str, plugin_id: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut skip = false;
    for line in raw.lines() {
        let t = line.trim();
        if t == "[]" {
            continue;
        }
        // Start of a block for the target: `- id: <id>` (plain or insert row).
        let is_target = t == format!("- id: {cordis_id}")
            || t == format!("id: {cordis_id}")
            || t == format!("- id: {plugin_id}")
            || t == format!("id: {plugin_id}");
        if is_target {
            skip = true;
            continue;
        }
        if skip {
            // Inside a target block: drop indented child lines and blank
            // separators; stop at the next top-level key.
            if t.is_empty() {
                continue;
            }
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            if indent > 0 {
                continue;
            }
            skip = false;
        }
        out.push(line.to_string());
    }

    let mut cleaned: Vec<String> = out;
    while cleaned.last().map(|l| l.trim().is_empty()) == Some(true) {
        cleaned.pop();
    }
    let mut result = cleaned.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    let body: String = result
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .collect::<Vec<_>>()
        .join("\n");
    if body.trim().is_empty() {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("[]\n");
    }
    result
}

/// Add or remove a `disabled: true` row for a cordis id in cordis.patch.yml.
fn set_disabled_row(raw: &str, cordis_id: &str, enabled: bool) -> String {
    // Remove any existing rows for this id (both plain and commented forms).
    let mut out: Vec<String> = Vec::new();
    let mut skip_block = false;
    for line in raw.lines() {
        let t = line.trim();
        // A top-level `[]` placeholder is dropped when we have any real entry
        // to write; it is kept only while the document stays empty.
        if t == "[]" {
            continue;
        }
        let is_target_id = t == format!("- id: {cordis_id}") || t == format!("id: {cordis_id}");
        if is_target_id {
            // Start of a block for this id; look ahead: if it is a pure
            // `disabled: true` block we drop it entirely.
            skip_block = true;
            continue;
        }
        if skip_block {
            // Inside the block: only `disabled:` and blank lines belong to it.
            if t == "disabled: true" || t == "disabled: false" || t.is_empty() {
                skip_block = false; // end of this small block
                continue;
            }
            // Block has other content (config etc.) — keep it, stop skipping.
            skip_block = false;
            out.push(line.to_string());
            continue;
        }
        out.push(line.to_string());
    }

    let mut cleaned: Vec<String> = out;
    // Trim trailing blank lines.
    while cleaned.last().map(|l| l.trim().is_empty()) == Some(true) {
        cleaned.pop();
    }

    if !enabled {
        // Append a fresh disable row (block sequence, never after `[]`).
        cleaned.push(String::new());
        cleaned.push(format!("- id: {cordis_id}"));
        cleaned.push("  disabled: true".to_string());
    }

    let mut result = cleaned.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    // If the document became empty again (everything removed), restore the
    // `[]` placeholder so the file stays a valid top-level array.
    let body: String = result
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .collect::<Vec<_>>()
        .join("\n");
    if body.trim().is_empty() {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("[]\n");
    }
    result
}

/// Which instance/profile a `dsh plugin` invocation targets.
struct PluginCliTarget<'a> {
    version_dir: &'a std::path::Path,
    home_path: &'a std::path::Path,
    profile: &'a str,
}

/// What the invocation does: a pnpm subcommand (`add` / `remove`), its
/// package spec, and the pnpm log level to forward.
struct PluginCliOp<'a> {
    subcommand: &'a str,
    spec: &'a str,
    loglevel: &'a str,
}

/// Runs one `dsh plugin --profile <name> <pnpm subcommand> <spec/id>` through
/// the instance's own CLI, streaming its output into the task log.
///
/// The launcher still prepares the two things the CLI does not: the
/// build-scripts opt-in (pnpm ≥10 `onlyBuiltDependencies` / pnpm 11
/// `allowBuilds`) and the profile `.npmrc` peer policy. When pnpm 11 blocks
/// build scripts it writes `set this to true or false` placeholders and fails
/// with ERR_PNPM_IGNORED_BUILDS; the placeholders are approved and the
/// invocation is retried once so native deps (node-pty, koffi, esbuild,
/// sharp…) actually build. The CLI prints the same advice for git-hosted
/// plugins, which this automates.
/// Runs one `dsh plugin --profile <name> remove <id>` synchronously and
/// returns its combined output on failure. The uninstall path is a plain
/// command now (no background task): short-lived, cancellable by dropping
/// the await on the frontend, with errors surfaced directly.
fn run_dsh_plugin_sync(
    state: &State<'_, AppState>,
    target: &PluginCliTarget<'_>,
    op: &PluginCliOp<'_>,
) -> Result<(), String> {
    let (version_dir, home_path, profile) = (target.version_dir, target.home_path, target.profile);
    let (subcommand, spec, loglevel) = (op.subcommand, op.spec, op.loglevel);
    let dir = profile_dir(home_path, profile);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 profile 目录失败: {e}"))?;
    ensure_build_scripts_allowed(&dir)?;
    // Never let a plugin's peers pull a second copy of a core package in.
    ensure_profile_npmrc(&dir)?;

    let pnpm_prog = ensure_pnpm_for_plugins(state)?;
    let what = format!("dsh plugin {subcommand}");

    // A node_modules tree linked from a *different* pnpm store makes pnpm
    // fail with ERR_PNPM_UNEXPECTED_STORE; relink proactively like the
    // installer path did.
    let store_dir = state.data_dir.join(".pnpm-store");
    if let Some(linked) = linked_store_dir(&dir) {
        if !store_paths_match(&linked, &store_dir.to_string_lossy()) {
            crate::log_info!("node_modules 链接自其他 pnpm store（{linked}），重新链接后重试");
            relink_profile_store_sync(state, target, &pnpm_prog)?;
        }
    }

    for attempt in 1..=2 {
        let mut args: Vec<String> = vec![subcommand.to_string(), spec.to_string()];
        args.extend(forwarded_pnpm_flags(state, loglevel, subcommand));
        let cmd = dsh_plugin_command(version_dir, home_path, profile, &args, &pnpm_prog)?;
        match run_command_sync(cmd, &what) {
            Ok(()) => return Ok(()),
            Err(out) if attempt == 1 && mentions_ignored_builds(&out) => {
                crate::log_info!("pnpm 拦截了依赖构建脚本，批准 allowBuilds 后重试");
                ensure_build_scripts_allowed(&dir)?;
            }
            Err(out) if attempt == 1 && mentions_unexpected_store(&out) => {
                crate::log_info!("pnpm 报告 store 位置不一致，重新链接后重试");
                relink_profile_store_sync(state, target, &pnpm_prog)?;
            }
            Err(out) => return Err(format!("{what} 失败: {out}")),
        }
    }
    unreachable!("attempt loop covers both attempts")
}

/// Runs a piped child command synchronously, returning the last meaningful
/// output lines on failure.
fn run_command_sync(mut cmd: std::process::Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what} 启动失败: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let tail: Vec<&str> = combined
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let summary = tail
        .iter()
        .rev()
        .take(3)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    Err(if summary.is_empty() {
        format!("退出码 {}", out.status)
    } else {
        summary
    })
}

fn mentions_unexpected_store(out: &str) -> bool {
    out.contains("ERR_PNPM_UNEXPECTED_STORE") || out.contains("Unexpected store location")
}

fn mentions_ignored_builds(out: &str) -> bool {
    out.contains("ERR_PNPM_IGNORED_BUILDS") || out.contains("Ignored build scripts")
}

/// Reads the store a profile's `node_modules` is currently linked from, via
/// the `storeDir` line pnpm records in `node_modules/.modules.yaml`.
fn linked_store_dir(profile_dir: &std::path::Path) -> Option<String> {
    let raw =
        std::fs::read_to_string(profile_dir.join("node_modules").join(".modules.yaml")).ok()?;
    for line in raw.lines() {
        if let Some(v) = line.trim().strip_prefix("storeDir:") {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Whether two store paths point at the same store. pnpm records the
/// versioned subdirectory (`<store>/v11`) while the launcher pins the base
/// dir, so a path containing the other as a prefix also counts as a match.
/// Checks whether two store paths match or one is an ancestor of the other.
fn store_paths_match(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim_end_matches('/').to_string();
    let (a, b) = (norm(a), norm(b));
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

/// Relinks a profile's `node_modules` onto the launcher's pinned store.
/// pnpm's own remedy for ERR_PNPM_UNEXPECTED_STORE is a plain reinstall,
/// which re-imports the lockfile packages from the new store without
/// touching package.json; routing it through `dsh plugin install` keeps the
/// CLI's bundle reconciliation in the loop.
fn relink_profile_store_sync(
    state: &State<'_, AppState>,
    target: &PluginCliTarget<'_>,
    pnpm_prog: &std::path::Path,
) -> Result<(), String> {
    let mut args: Vec<String> = vec!["install".to_string()];
    args.extend(forwarded_pnpm_flags(state, "warn", "install"));
    let cmd = dsh_plugin_command(
        target.version_dir,
        target.home_path,
        target.profile,
        &args,
        pnpm_prog,
    )?;
    run_command_sync(cmd, "dsh plugin install（重新链接 store）")
        .map_err(|e| format!("dsh plugin install（重新链接 store） 失败: {e}"))
}


/// Builds a `dsh plugin --profile <name> <pnpm args…>` invocation for an
/// instance's own CLI version.
///
/// Profile plugin management is a CLI-private flow: `dsh plugin` initializes
/// the profile when needed, forwards the remaining arguments to pnpm with
/// cwd = the profile directory, and then reconciles `dsh.profile.bundles`
/// against the *installed* state (a dependency whose package declares
/// `dsh.bundle.patch` joins the layer stack; one that no longer does leaves
/// it). Driving pnpm ourselves would produce a tree the CLI does not expect
/// and would leave the layer list to be guessed at, so every install and
/// removal goes through the CLI of the version that instance runs.
///
/// The CLI resolves pnpm from PATH, so the launcher's pinned pnpm
/// (`REQUIRED_PNPM_MAJOR`) is prepended to PATH: the pin then also applies
/// inside the CLI's own pnpm invocation.
fn dsh_plugin_command(
    version_dir: &std::path::Path,
    home_path: &std::path::Path,
    profile: &str,
    pnpm_args: &[String],
    pnpm_prog: &std::path::Path,
) -> Result<std::process::Command, String> {
    let bin = crate::process::version_bin(version_dir);
    if !crate::process::version_bin_ready(version_dir) {
        return Err(format!(
            "版本安装不完整（缺少 {}），请重新安装该 DSH 版本",
            bin.display()
        ));
    }

    let mut cmd = std::process::Command::new(crate::process::node());
    cmd.arg(&bin)
        .arg("plugin")
        .arg("--profile")
        .arg(profile)
        .args(pnpm_args)
        .env("DSH_HOME", home_path)
        // The launcher can never answer an interactive prompt: pnpm aborts
        // with ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY when it needs to
        // purge a modules dir (store/virtual-store relink) without a TTY.
        // CI=true makes pnpm treat the run as non-interactive instead.
        .env("CI", "true");

    // Prepend the pinned pnpm's directory so the CLI's `spawnSync("pnpm")`
    // picks it up instead of whatever major is on the user's PATH.
    if let Some(pnpm_dir) = pnpm_prog.parent() {
        if !pnpm_dir.as_os_str().is_empty() {
            let existing = std::env::var_os("PATH").unwrap_or_default();
            let mut entries = vec![pnpm_dir.to_path_buf()];
            entries.extend(std::env::split_paths(&existing));
            match std::env::join_paths(entries) {
                Ok(joined) => {
                    cmd.env("PATH", joined);
                }
                Err(e) => {
                    crate::log_warn!("拼接 PATH 失败，沿用系统 PATH: {e}");
                }
            }
        }
    }

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    Ok(cmd)
}

/// Common pnpm flags forwarded through `dsh plugin` (shared store, network
/// robustness, optional registry mirror). `--prefix` is deliberately absent:
/// the CLI already runs pnpm with cwd = the profile directory, and passing a
/// prefix would break that contract.
///
/// The fetch/network flags only exist on download commands (`add` /
/// `install`): `pnpm remove` rejects them outright ("Unknown options:
/// 'fetch-timeout', …") and would fail before touching anything.
fn forwarded_pnpm_flags(
    state: &State<'_, AppState>,
    loglevel: &str,
    subcommand: &str,
) -> Vec<String> {
    let store_dir = state.data_dir.join(".pnpm-store");
    let mut args: Vec<String> = vec![
        "--store-dir".to_string(),
        store_dir.to_string_lossy().to_string(),
        format!("--loglevel={loglevel}"),
    ];
    if subcommand != "remove" {
        args.extend([
            "--fetch-timeout".to_string(),
            "300000".to_string(),
            "--fetch-retries".to_string(),
            "5".to_string(),
            "--fetch-retry-maxtimeout".to_string(),
            "120000".to_string(),
            "--network-concurrency".to_string(),
            "4".to_string(),
        ]);
    }
    if let Ok(registry) = std::env::var("DSH_NPM_REGISTRY") {
        let registry = registry.trim().to_string();
        if !registry.is_empty() {
            args.push("--registry".to_string());
            args.push(registry);
        }
    }
    args
}

/// Pins `auto-install-peers=false` in a profile's `.npmrc`.
///
/// A DSH profile must resolve nothing from the `@deepseek-ai` core scope —
/// core comes from the CLI's own dependency tree. With auto-install-peers on
/// (a common global pnpm setting), installing a plugin whose peers include a
/// core package drops a second copy of that core package into the profile,
/// which is the duplicated-Symbol failure the doctor check reports. Writing
/// the setting per profile makes the install independent of the user's global
/// pnpm configuration.
pub(crate) fn ensure_profile_npmrc(dir: &std::path::Path) -> Result<(), String> {
    const KEY: &str = "auto-install-peers";
    let path = dir.join(".npmrc");
    let raw = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("读取 .npmrc 失败: {e}"))?
    } else {
        String::new()
    };

    let mut lines: Vec<String> = raw.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    let mut changed = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        if k.trim() != KEY {
            continue;
        }
        found = true;
        if v.trim() != "false" {
            *line = format!("{KEY}=false");
            changed = true;
        }
    }
    if !found {
        // Keep a short rationale in the file: it is user-visible state.
        if !lines.is_empty() && !lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push("# DSH: core packages come from the CLI dependency tree;".to_string());
        lines.push("# a profile must never resolve its own copy.".to_string());
        lines.push(format!("{KEY}=false"));
        changed = true;
    }
    if changed {
        let mut out = lines.join("\n");
        out.push('\n');
        std::fs::write(&path, out).map_err(|e| format!("写入 .npmrc 失败: {e}"))?;
    }
    Ok(())
}

/// Make sure a profile's pnpm-workspace.yaml opts into dependency build
/// scripts on both pnpm 10 (onlyBuiltDependencies) and pnpm 11 (allowBuilds).
///
/// pnpm 11 writes `allowBuilds: <name>: set this to true or false` for every
/// dependency whose build script it ignored, then fails the install with
/// ERR_PNPM_IGNORED_BUILDS. This converts those placeholders to `true` and
/// keeps the old field around for pnpm ≤10, so a subsequent install actually
/// runs the native build scripts (node-pty, koffi, esbuild, sharp, …).
pub(crate) fn ensure_build_scripts_allowed(dir: &std::path::Path) -> Result<(), String> {
    let ws_manifest = dir.join("pnpm-workspace.yaml");
    let raw = if ws_manifest.exists() {
        std::fs::read_to_string(&ws_manifest)
            .map_err(|e| format!("读取 pnpm-workspace.yaml 失败: {e}"))?
    } else {
        String::new()
    };

    // Base document: `packages` is required for pnpm to treat the dir as a
    // workspace (needed for allowBuilds to be read from this file).
    let mut lines: Vec<String> = if raw.trim().is_empty() {
        vec!["packages:".to_string(), "  - .".to_string()]
    } else {
        raw.lines().map(|l| l.to_string()).collect()
    };

    // 1. Convert pnpm-11 placeholder values ("set this to true or false") to
    //    real booleans so the next install builds those packages.
    let mut changed = false;
    for line in lines.iter_mut() {
        if line.contains("set this to true or false") {
            *line = line.replace("set this to true or false", "true");
            changed = true;
        }
    }

    // 2. Ensure the legacy `onlyBuiltDependencies: ['*']` block exists
    //    (pnpm ≤10 reads only this field).
    let joined = lines.join("\n");
    if !joined.contains("onlyBuiltDependencies") {
        lines.push(String::new());
        lines.push("onlyBuiltDependencies:".to_string());
        lines.push("  - '*'".to_string());
        changed = true;
    }

    // 3. Ensure an `allowBuilds:` section exists so pnpm 11 has somewhere to
    //    record newly-ignored builds (it auto-appends entries on failure).
    if !lines
        .iter()
        .any(|l| l.trim_start().starts_with("allowBuilds:"))
    {
        lines.push(String::new());
        lines.push("allowBuilds:".to_string());
        changed = true;
    }

    if changed {
        let out = lines.join("\n");
        if !out.ends_with('\n') {
            lines.push(String::new());
        }
        std::fs::write(&ws_manifest, lines.join("\n"))
            .map_err(|e| format!("写入 pnpm-workspace.yaml 失败: {e}"))?;
    }
    Ok(())
}

/// Resolve a pnpm executable on the required major, bootstrapping the
/// pinned one into the data dir when needed (same policy as version
/// installs, but synchronous and logging to the app log instead of a task).
fn ensure_pnpm_for_plugins(state: &State<'_, AppState>) -> Result<std::path::PathBuf, String> {
    use std::process::Command;
    let major = crate::tasks::REQUIRED_PNPM_MAJOR;
    let probe = |prog: &std::path::Path| -> bool {
        Command::new(prog)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| crate::tasks::pnpm_major_pub(&String::from_utf8_lossy(&o.stdout)))
            .map(|m| m == major)
            .unwrap_or(false)
    };
    let system = std::path::PathBuf::from(crate::process::pnpm());
    if probe(&system) {
        return Ok(system);
    }
    let tools_dir = state.data_dir.join("tools");
    let local = tools_dir.join("pnpm");
    if local.exists() && probe(&local) {
        return Ok(local);
    }
    std::fs::create_dir_all(&tools_dir).map_err(|e| format!("创建工具目录失败: {e}"))?;
    crate::log_info!("正在安装 DSH profile 所需的 pnpm@{major}…");
    let out = Command::new(crate::process::npm())
        .args(["install", "--global", "--prefix"])
        .arg(&tools_dir)
        .arg(format!("pnpm@{major}"))
        .output()
        .map_err(|e| format!("pnpm 安装启动失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.lines().last().unwrap_or("未知错误").to_string();
        return Err(format!("pnpm 安装失败: {last}"));
    }
    if !local.exists() {
        return Err(format!("pnpm 安装完成但未找到可执行文件: {}", local.display()));
    }
    Ok(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cordis_id_of_strips_scope_and_org() {
        assert_eq!(cordis_id_of("@dsh-plugin/dsh-auxiliary"), "dsh-auxiliary");
        assert_eq!(cordis_id_of("@dsh-external/dsh-sidechain"), "dsh-sidechain");
        assert_eq!(cordis_id_of("dsh-better-sidebar"), "dsh-better-sidebar");
        assert_eq!(cordis_id_of("@canglongcl/dsh-web-review"), "dsh-web-review");
    }

    #[test]
    fn store_paths_match_handles_versioned_subdir_and_slashes() {
        let base = "/Users/x/Library/Application Support/in.dsh-plug.dsh-launcher/.pnpm-store";
        // `.modules.yaml` records the versioned subdir pnpm derived from the
        // pinned base.
        assert!(store_paths_match(&format!("{base}/v11"), base));
        // A trailing separator is equivalent.
        assert!(store_paths_match(
            "/Users/x/Library/Application Support/in.dsh-plug.dsh-launcher/.pnpm-store/v11/",
            base
        ));
        // A genuinely different store (the user's global one) mismatches.
        assert!(!store_paths_match("/Users/x/Library/pnpm/store/v11", base));
    }

    #[test]
    fn linked_store_dir_reads_modules_yaml() {
        let dir = std::env::temp_dir().join(format!("dsh-test-modules-{}", uuid::Uuid::new_v4()));
        let nm = dir.join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(
            nm.join(".modules.yaml"),
            "hoist: true\nstoreDir: C:\\Users\\x\\AppData\\Local\\pnpm\\store\\v11\nvirtualStoreDir: ...\n",
        )
        .unwrap();
        assert_eq!(
            linked_store_dir(&dir).as_deref(),
            Some("C:\\Users\\x\\AppData\\Local\\pnpm\\store\\v11")
        );
        std::fs::remove_dir_all(&dir).ok();
        // Missing file → None (fresh profile, nothing to relink).
        assert_eq!(linked_store_dir(&dir), None);
    }

    #[test]
    fn set_disabled_row_adds_and_removes() {
        let raw = "# comment\n- id: other-plugin\n  config:\n    a: 1\n";
        // Add a disable row for dsh-auxiliary.
        let out = set_disabled_row(raw, "dsh-auxiliary", false);
        assert!(out.contains("- id: dsh-auxiliary"), "out: {out}");
        assert!(out.contains("  disabled: true"), "out: {out}");
        // The unrelated block must be preserved.
        assert!(out.contains("other-plugin"), "out: {out}");
        assert!(out.contains("config"), "out: {out}");
        assert!(out.contains("a: 1"), "out: {out}");

        // Remove it again -> back to the original content.
        let back = set_disabled_row(&out, "dsh-auxiliary", true);
        assert!(!back.contains("dsh-auxiliary"), "back: {back}");
        assert!(back.contains("other-plugin"), "back: {back}");
        assert!(back.contains("config"), "back: {back}");
    }

    #[test]
    fn set_disabled_row_replaces_existing() {
        let raw = "- id: dsh-auxiliary\n  disabled: true\n";
        let out = set_disabled_row(raw, "dsh-auxiliary", true);
        assert!(!out.contains("dsh-auxiliary"), "out: {out}");
        // Re-disable after removal.
        let out2 = set_disabled_row(&out, "dsh-auxiliary", false);
        assert!(out2.contains("- id: dsh-auxiliary"), "out2: {out2}");
        assert!(out2.contains("  disabled: true"), "out2: {out2}");
    }

    #[test]
    fn read_disabled_ids_parses_blocks() {
        let dir = std::env::temp_dir().join(format!("dsh-plugins-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("cordis.patch.yml"),
            "# header\n- id: ui-dsh-aionui-panel\n  disabled: true\n\n- id: live-stats\n  disabled: true\n\n- id: keep\n  config:\n    x: 1\n",
        )
        .unwrap();
        let set = read_disabled_ids(&dir);
        assert!(set.contains("ui-dsh-aionui-panel"));
        assert!(set.contains("live-stats"));
        assert!(!set.contains("keep"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_cordis_rows_removes_insert_and_disabled_blocks() {
        // A plugin mounted via an insert row plus a disabled row for another
        // plugin must leave the other plugin intact.
        let raw = "# header\n- insert:\n    - id: dsh-auxiliary\n      name: '@dsh-plugin/dsh-auxiliary'\n\n- id: dsh-thought-buddy\n  disabled: true\n\n- id: keep\n  config:\n    x: 1\n";
        let out = strip_cordis_rows(raw, "dsh-auxiliary", "@dsh-plugin/dsh-auxiliary");
        assert!(!out.contains("dsh-auxiliary"), "insert row removed: {out}");
        assert!(out.contains("dsh-thought-buddy"), "other block kept: {out}");
        assert!(out.contains("keep"), "config block kept: {out}");
        assert!(out.contains("x: 1"), "config content kept: {out}");
    }

    #[test]
    fn strip_cordis_rows_restores_placeholder_when_empty() {
        let raw = "# header\n- id: dsh-auxiliary\n  disabled: true\n";
        let out = strip_cordis_rows(raw, "dsh-auxiliary", "@dsh-plugin/dsh-auxiliary");
        assert!(out.contains("[]"), "placeholder restored: {out}");
        assert!(!out.contains("dsh-auxiliary"), "entry removed: {out}");
    }

    // Live network smoke tests (skipped by default; run with
    // `cargo test plugins::tests::live_ -- --ignored`).
    #[test]
    fn ensure_build_scripts_allowed_converts_placeholders_and_adds_sections() {
        let dir = std::env::temp_dir().join(format!("dsh-plugins-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Fresh profile: no workspace file yet -> packages + both sections.
        ensure_build_scripts_allowed(&dir).unwrap();
        let fresh = std::fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        assert!(fresh.contains("packages:"), "fresh: {fresh}");
        assert!(fresh.contains("onlyBuiltDependencies"), "fresh: {fresh}");
        assert!(fresh.contains("allowBuilds:"), "fresh: {fresh}");

        // pnpm 11 left a placeholder behind after ERR_PNPM_IGNORED_BUILDS.
        std::fs::write(
            dir.join("pnpm-workspace.yaml"),
            "packages:\n  - .\nallowBuilds:\n  node-pty: set this to true or false\n",
        )
        .unwrap();
        ensure_build_scripts_allowed(&dir).unwrap();
        let fixed = std::fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        assert!(fixed.contains("node-pty: true"), "fixed: {fixed}");
        assert!(
            !fixed.contains("set this to true or false"),
            "fixed: {fixed}"
        );
        // Legacy section added without clobbering existing content.
        assert!(fixed.contains("onlyBuiltDependencies"), "fixed: {fixed}");

        // Idempotent: second run leaves the file unchanged.
        let before = std::fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        ensure_build_scripts_allowed(&dir).unwrap();
        let after = std::fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        assert_eq!(before, after, "must be idempotent");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ensure_profile_npmrc_pins_auto_install_peers_false() {
        let dir = std::env::temp_dir().join(format!("dsh-npmrc-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let npmrc = dir.join(".npmrc");

        // Fresh profile: the key is written.
        ensure_profile_npmrc(&dir).unwrap();
        let fresh = std::fs::read_to_string(&npmrc).unwrap();
        assert!(fresh.contains("auto-install-peers=false"), "fresh: {fresh}");

        // Idempotent.
        ensure_profile_npmrc(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&npmrc).unwrap(), fresh);

        // An opposite existing value is normalized, other keys are preserved.
        std::fs::write(
            &npmrc,
            "registry=https://example.com/\nauto-install-peers=true\n",
        )
        .unwrap();
        ensure_profile_npmrc(&dir).unwrap();
        let fixed = std::fs::read_to_string(&npmrc).unwrap();
        assert!(fixed.contains("auto-install-peers=false"), "fixed: {fixed}");
        assert!(!fixed.contains("auto-install-peers=true"), "fixed: {fixed}");
        assert!(
            fixed.contains("registry=https://example.com/"),
            "other keys must survive: {fixed}"
        );

        // A commented-out key is not treated as set.
        std::fs::write(&npmrc, "# auto-install-peers=true\n").unwrap();
        ensure_profile_npmrc(&dir).unwrap();
        let commented = std::fs::read_to_string(&npmrc).unwrap();
        assert!(
            commented.contains("\nauto-install-peers=false"),
            "commented: {commented}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
