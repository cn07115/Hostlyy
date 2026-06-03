# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.7] - 2026-06-04

### Performance
- **WebDAV sync: content-hash dedup.** Added `last_hash: Option<String>`
  to `ProfileMetadata` and `sha2` to `Cargo.toml`. Before any PUT, the
  client computes the SHA-256 of the local file and skips the upload
  (no HTTP request at all) if the hash matches the one stored in
  `config.sync.json` from the last successful upload. This fixes a
  real-world pathology where local filesystem mtime precision (sub-
  second) differs from HTTP `Last-Modified` precision (whole seconds),
  causing identical content to be re-uploaded on every sync. The new
  hash is persisted to `config.sync.json` after each upload, and is
  also reset on download (so a fresh pull replaces the local hash).

### Features
- **Theme: "Follow system" mode.** Added a third radio option next to
  Light and Dark. When selected, the app subscribes to Tauri's
  `WebviewWindow::onThemeChanged` and re-applies the OS theme on
  every change. Setting is persisted in `LocalConfig.theme` and
  accepts `"light" | "dark" | "system"`.

### Fixes
- **Light theme: hardcoded `.type-switch` background.** The segmented
  control's groove was `background: #161b22` (dark gray) regardless
  of the active theme, making the appearance look identical in both
  modes. Now uses `var(--input-bg)` (`#f9fafb` light / dark
  translucent dark) so it actually responds to theme switching.
- **Light theme: hardcoded `.footer-text-btn:hover` border.** Replaced
  the hardcoded `#8b949e` with `var(--text-dim)` and the hardcoded
  hover background with `var(--hover-bg)` for proper theme response.

## [1.2.6] - 2026-06-04

### Performance
- **WebDAV sync: parallel I/O.** Wrapped the upload/download/delete
  loops in `perform_sync` (`5a`/`5b`/`5c`) in `std::thread::scope`,
  with per-collector `Arc<Mutex<Vec<String>>>` to merge results. N
  profile files now transfer in ~1 RTT instead of N×RTT (subject to
  the WebDAV server's per-connection concurrency limit).

### Fixes
- **WebDAV sync: false-positive staleness warning on first sync.**
  `check_staleness` previously fired for every file on the first sync
  because `remote_mtime == 0` is treated as epoch, making the
  local-vs-remote delta ~20607 days, exceeding the 30-day threshold.
  Returns `None` when `remote_mtime == 0`.
- **WebDAV sync: verbose toast.** The sync completion toast was
  appending all 30-day warnings (now empty) and the full per-file
  summary, producing a wall of text for N profiles. Toast now shows
  just `同步完成 (上传 N 个)` / `部分完成 (X 个错误)`; full result
  is logged to `console.log` / `console.error` for inspection via
  DevTools (F12).

## [1.2.5] - 2026-06-03

### Fixes
- **WebDAV sync: file paths.** `perform_sync` was MKCOL-ing
  `<base>/hostly` and `<base>/hostly/profiles` correctly, but
  PUT/GET-ing files at `<base>/config.sync.json` and
  `<base>/profiles/<id>.txt` (i.e. to the WebDAV root), leaving
  the `hostly/` directories empty. All PUT/GET URLs now use
  `REMOTE_DIR` / `PROFILES_REMOTE_DIR` constants; the `hostly/`
  filter is updated accordingly. Existing users will have orphan
  files at the WebDAV root from previous syncs that need to be
  cleaned up via a WebDAV client.

## [1.2.4] - 2026-06-03

### Fixes
- **Settings: misleading "未配置" status.** `save_webdav_config`
  intentionally clears `webdav_last_status` on every save (to avoid
  showing stale "ok" after the config changes), but the frontend
  treated `last_status == null` as "not configured". The display
  logic now uses the backend's `configured: bool` for the
  未配置 check, and shows "已配置,未同步" when configured but no
  sync has run.

## [1.2.3] - 2026-06-03

### Fixes
- **WebDAV: test connection no longer deletes the keychain password.**
  The "test connection" button was auto-saving with current DOM
  values before testing. Since the "save" button intentionally
  clears the password field for security, the test button re-sent
  an empty password, which routed to `delete_credentials` and
  removed the freshly-stored keychain entry. The test button no
  longer auto-saves; the backend now returns a friendly "请先点击
  保存配置" error when the user tests before saving.
- **WebDAV: `TypeError: t.summary is not a function`.** `SyncResult`
  is serialized across the Tauri IPC boundary as a plain JSON
  object; Rust methods don't survive serialization. Replaced the
  call with an in-JS `formatSyncSummary(r)` that re-implements the
  same logic.
- **WebDAV: `test_webdav_connection` error message.** Replaced the
  bare "WebDAV URL 未配置" with "请先填写并点击「保存配置」"
  to direct users to the right action.

## [1.2.2] - 2026-06-03

### Features
- **WebDAV sync: webdav-error event emission.** Background sync paths
  (startup pull, periodic pull, scheduler loop) now emit a
  `webdav-error` Tauri event on failure instead of silently
  `eprintln!`-ing. Frontend listens and shows a toast.
- **WebDAV sync: silent skip when unconfigured.** `sync_now_internal`
  returns `Ok(None)` when URL/username are missing, so the manual
  "立即同步" button shows an info toast "WebDAV 未配置,跳过了同步"
  instead of an error.
- **WebDAV sync: distinct credentials error.** The sync path now
  differentiates "credentials missing in keychain" (returns
  "凭证读取失败: ...") from generic sync errors, and writes a
  structured `error: credentials: ...` to `webdav_last_status`.

### Fixes
- **URL open: no more console flash.** `hostly_open_url` was using
  `cmd /C start <url>` on Windows, which briefly allocated a
  console. Switched to the `open` crate's `open::that_detached`.
- **Settings: `.pane-title` spacing.** Bumped `padding-bottom` from
  10px to 14px and added `.pane-title + .form-group { margin-top:
  16px }` to give the divider line proper breathing room before
  the first control.
- **GitHub link: points to the fork repo.** Was pointing to
  `zengyufei/Hostlyy` (typo, upstream repo) — now points to
  `cn07115/Hostlyy`.

## [1.2.1] - 2026-06-03

### Features
- **WebDAV auto-sync: reactive debounce + periodic pull.** Every
  mutation (create/delete/rename/toggle/multi-select/update-remote)
  calls `webdav::schedule_sync()`, which sets a 5-second deadline.
  A single background task wakes every 500ms and runs `sync_now`
  when the deadline elapses. This batches bursts of rapid changes
  into one network round trip. On startup, a 3-second-delayed pull
  ensures the device starts with the latest remote state; a
  periodic pull every 5 minutes catches changes from offline
  devices.

## [1.2.0] - 2026-06-03

### Features
- **WebDAV multi-device sync.** New "云端同步 (WebDAV)" pane in
  settings. URL/username/password inputs + "测试连接" + "立即同步"
  buttons. Conflicts resolved last-write-wins via HTTP
  `Last-Modified` headers. Credentials stored in the OS keychain
  (Windows Credential Manager / macOS Keychain / Linux Secret
  Service) via the `keyring` crate; never written to config files.
  Only `config.sync.json` and `profiles/<id>.txt` are synced —
  system settings (`config.local.json`) stay local.
- **Storage split.** `config.json` is split into `config.local.json`
  (theme, window config, sidebar width, auto-start, close
  behavior, WebDAV config) and `config.sync.json` (multi-select,
  profiles, active profile ids). Auto-migration on first load:
  if the legacy `config.json` exists, it's split into the two new
  files, the legacy file is kept as backup. `AppConfig` is the
  merged view returned to the frontend — no UI changes needed.

## [1.1.0] - 2026-06-03

### Features
- **Cross-platform auto-elevation.** Windows: `IsUserAnAdmin` +
  `ShellExecuteExW` + `runas` (FFI). macOS: `osascript -e 'do
  shell script ... with administrator privileges'`. Linux: `pkexec`
  with `sudo` fallback.
- **Cross-platform auto-start.** Replaced the hand-rolled
  Windows-registry-only implementation with `tauri-plugin-autostart`
  (HKCU on Windows, `~/Library/LaunchAgents` plist on macOS,
  `~/.config/autostart` .desktop on Linux).
- **Close button: in-app confirmation modal.** The previous
  system `ask()` dialog silently failed in Tauri 2. Replaced with
  a custom modal offering "退出 / 最小化到托盘 / 记住本次选择".
  CSS selector was missing `#close-confirm-overlay` from the
  `.modal-overlay` rule, which is why it previously appeared to do
  nothing.
- **GitHub Actions multi-platform CI.** `.github/workflows/build.yml`
  matrix: `windows-latest` / `macos-latest` (aarch64 + x86_64) /
  `ubuntu-22.04`. 14-day artifact retention. Auto-release on `v*`
  tag push via `softprops/action-gh-release@v2`; release notes
  extracted from `CHANGELOG.md` via `sed`. Fork-friendly: works on
  forks with proper Actions permissions.

### Fixes
- **Console window flash on launch.** Root cause: the
  `relaunch_as_admin_if_needed` path spawned `net session` as a
  console-subsystem child, which allocated a console on the
  GUI process. Fixed by using `ShellExecuteExW` + `runas` directly.
- **Auto-start silently broken on macOS/Linux.** Root cause: the
  original `autostart.rs` was guarded with `#[cfg(target_os =
  "windows")]`, so non-Windows platforms returned `false`
  unconditionally. Fixed by switching to `tauri-plugin-autostart`.
- **Missing Cargo.toml dependencies.** `tray-icon` feature and
  `tauri-plugin-autostart` were used in code but not declared.
  README claimed tray support worked, but the feature was never
  enabled at build time.
