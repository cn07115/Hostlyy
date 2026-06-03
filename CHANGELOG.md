# 更新日志 (Changelog)

本项目的所有重要变更都会记录在此文件中。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

## [1.2.8] - 2026-06-04

### 修正 (Fixed)
- **修 v1.2.7 编译失败(发布未生效)**:`ProfileMetadata` 加 `last_hash: Option<String>` 字段后,3 处使用显式 initializer 语法(`ProfileMetadata { id, name, ..., update_interval }`)的地方没加新字段,Rust 报 E0063 "missing field `last_hash`",4/4 平台 build 失败。补全 `last_hash: None` 到 storage.rs:352/365/508 三处 initializer。本地 `cargo check` 验证后 force-move v1.2.7 tag 到修复 commit `76e7416`,CI 重跑 5/5 绿,release 真正发布。

### 文档 (Documentation)
- **CHANGELOG 重写为中文**:之前 v1.2.2 - v1.2.7 的英文条目改写为中文,跟项目自身语言保持一致(界面、commit message 都是中文)。结构用 `Keep a Changelog` 的 Added/Changed/Fixed/Removed/Security,但每条描述保持简练,不写流水账。

## [1.2.7] - 2026-06-04

### 新增 (Added)
- **WebDAV 内容去重(SHA-256)**:本地 profile 文件未实际改动时,**完全不发 PUT 请求**。修了 mtime 精度差(local FS μs/ns, HTTP Last-Modified 1s)导致每次 sync 都重传同样内容的 bug。新增 `sha2` 依赖,`ProfileMetadata` 加 `last_hash: Option<String>` 字段(`#[serde(default)]` 兼容老 config),上传前算 hash 跟 `last_hash` 比对,匹配直接跳过 PUT。下载后重置 hash。
- **主题「跟随系统」选项**:明亮/深色之外加第三个 radio。选 `system` 时通过 Tauri 2 `WebviewWindow.onThemeChanged` 订阅 OS 主题,实时跟系统切换。存储用 `LocalConfig.theme = "light"/"dark"/"system"`。

### 修复 (Fixed)
- **浅色主题 .type-switch background 硬编码**:`background: #161b22` 之前跟主题无关,切到浅色还是深色槽背景。改用 `var(--input-bg)`,浅色 `#f9fafb`,深色 translucent dark。
- **浅色主题 .footer-text-btn:hover border 硬编码**:同上,改用 `var(--text-dim)`。
- **v1.2.7 编译错误(后续修复)**:见 v1.2.8 修正条目。v1.2.7 的 release 页面是 force-push 后的修复版本。

## [1.2.6] - 2026-06-04

### 性能 (Performance)
- **WebDAV sync 并行化**:`perform_sync` 的 5a/5b/5c 三个循环从串行 HTTP 改成 `std::thread::scope` 并行,每个文件独立 goroutine 跑 PUT/GET/DELETE。每个结果收集器(`Arc<Mutex<Vec<String>>>`)独立写、无锁竞争。N 个文件从 N×RTT 降到 ~1×RTT(实际受 WebDAV server 并发限制)。

### 修复 (Fixed)
- **首次 sync staleness 假阳性**:`check_staleness` 之前对所有 `remote_mtime == 0` 的文件都警告(从 epoch 到 2026 是 20607 天,大于 30 天阈值)。`remote_mtime == 0` 时直接返回 None,只在真冲突时警告。
- **同步完成 toast 太啰嗦**:之前拼「同步完成: 上传 N 个 | ⚠ hostly/profiles/<id>.txt: ...」一长串。改为「同步完成 (上传 N 个)」/「部分完成 (X 个错误)」,完整 SyncResult 改 log 到 `console.log` / `console.error`,F12 开 devtools 看。

## [1.2.5] - 2026-06-03

### 修复 (Fixed)
- **WebDAV 文件路径错位**:`perform_sync` 正确 MKCOL 了 `<base>/hostly` 和 `<base>/hostly/profiles`,但 PUT/GET 文件时 URL 没加 `hostly/` 前缀,文件全落到 WebDAV 根目录。所有 PUT/GET URL 改用 `REMOTE_DIR`/`PROFILES_REMOTE_DIR` 常量,filter `url.contains("/profiles/")` 同步改成 `/hostly/profiles/`。**注意**:旧版本写到根目录的 orphan 文件需要 WebDAV 客户端手动清理。

## [1.2.4] - 2026-06-03

### 修复 (Fixed)
- **「未配置」状态显示误导**:`save_webdav_config` 每次保存都把 `webdav_last_status` 清成 None(本意是配置改了清掉旧状态),但前端把 `last_status == null` 解读为「未配置」。改为用后端 `SyncStatus.configured` 字段(`webdav_url.is_some() && webdav_username.is_some()`)判断「未配置」,`last_status == null` 改显示「已配置,未同步」,区分清楚。

## [1.2.3] - 2026-06-03

### 修复 (Fixed)
- **测试连接删 keychain 密码**:「测试连接」按钮原本会先调 `save_webdav_config` 再测试,「保存」按钮清空了 DOM 密码框(防泄漏)导致测试按钮读到空 password,后端 `save_credentials(_, "")` 走「空密码=删 entry」分支把刚存的密码删了。测试按钮不再 auto-save;未配置时后端返回「请先填写并点击 保存配置」。
- **`TypeError: t.summary is not a function`**:`SyncResult::summary()` 是 Rust 方法,Tauri IPC 跨进程只序列字段不带方法。前端同步按钮调用 `result.summary()` 报错。改用 JS helper `formatSyncSummary(r)` 复算。
- **测试连接未配置错误信息**:「WebDAV URL 未配置」→「请先填写并点击「保存配置」」,引导用户走正确流程。

## [1.2.2] - 2026-06-03

### 新增 (Added)
- **WebDAV 错误事件**:后台 sync 路径(启动拉取、周期拉取、调度器循环)失败时 emit `webdav-error` Tauri 事件,前端监听 toast 提示,不再 `eprintln!` 静默吞错。
- **未配置静默跳过**:`sync_now_internal` 未配置时返回 `Ok(None)`,前端手动同步按钮显示 info toast「WebDAV 未配置,跳过了同步」,不再弹错误。
- **凭证错误分类**:凭证缺失/读取失败单独错误信息(「凭证读取失败: ...」),结构化写入 `webdav_last_status`。

### 修复 (Fixed)
- **URL 打开不再闪 cmd 窗口**:`hostly_open_url` 从 `cmd /C start <url>` 换成 `open::that_detached`(跨平台,Windows 不再临时分配控制台)。
- **设置面板标题间距**:`.pane-title` `padding-bottom` 从 10px → 14px,加 `.pane-title + .form-group { margin-top: 16px }`,分隔线和下方第一个控件之间有合理呼吸空间。
- **GitHub 链接指向 fork repo**:之前指向 `zengyufei/Hostlyy`(拼写错误 + 上游仓库)→ 改为 `cn07115/Hostlyy`。

## [1.2.1] - 2026-06-03

### 新增 (Added)
- **WebDAV 自动同步(响应式防抖 + 周期拉取)**:每次本地改动(创建/删除/重命名/切换/多选/更新 remote)调 `webdav::schedule_sync()` 设 5s deadline,后台任务每 500ms tick 检查 deadline 触发 sync,把突发改动合并成一次网络往返。启动 3s 拉取一次(设备打开就拿到最新远端),每 5 分钟周期拉取(兜底多设备同步)。

## [1.2.0] - 2026-06-03

### 新增 (Added)
- **WebDAV 多设备同步**:设置页加「云端同步 (WebDAV)」面板,填 URL/用户名/密码即可启用。多设备间同步 hosts 配置(只同步 `config.sync.json` + `profiles/*.txt`,系统设置 `config.local.json` 不同步)。冲突策略 last-write-wins(基于 HTTP `Last-Modified`)。凭证存系统 keychain(Windows Credential Manager / macOS Keychain / Linux Secret Service),不写 config 文件。
- **存储拆分**:`config.json` 拆成 `config.local.json`(系统设置)+ `config.sync.json`(hosts 元数据)。启动时自动迁移老 config。`AppConfig` 仍作为合并视图返回前端,UI 无需改。

## [1.1.0] - 2026-06-03

### 新增 (Added)
- **跨平台自动提权**:Windows `IsUserAnAdmin` + `ShellExecuteExW(runas)`,macOS `osascript with administrator privileges`,Linux `pkexec`(polkit)/`sudo` fallback。修了原 `net session` 方案在 Windows 闪 cmd 窗口的问题。
- **跨平台开机自启**:从手写 Windows 注册表方案换到 `tauri-plugin-autostart`(HKCU / `~/Library/LaunchAgents` / `~/.config/autostart` 三平台统一)。修了原 `#[cfg(windows)]` 守卫导致 macOS/Linux 不生效的 bug。
- **关闭按钮 in-app 确认弹窗**:系统 `ask()` 在 Tauri 2 静默失败,换成自定义 modal 提供「退出 / 最小化到托盘 / 记住本次选择」。修了 CSS selector 缺 `#close-confirm-overlay` 导致弹窗不显示的问题。
- **GitHub Actions 多平台 CI**:`.github/workflows/build.yml` 矩阵:windows-latest / macos-latest (aarch64 + x86_64) / ubuntu-22.04,14 天 artifact 保留。`v*` tag push 自动通过 `softprops/action-gh-release@v2` 发 release,release notes 从 `CHANGELOG.md` 提取。fork 友好。

### 修复 (Fixed)
- 启动时 cmd 窗口一闪而过(原 `relaunch_as_admin` 调 `net session` 子进程,Windows 给控制台子系统子进程分配控制台)。
- macOS/Linux 自启动按钮无效(原 `autostart.rs` 有 `#[cfg(target_os = "windows")]` 守卫,非 Windows 平台直接 return false)。
- `Cargo.toml` 漏 `tray-icon` feature 和 `tauri-plugin-autostart` 依赖(README 吹有托盘但 feature 没开,本地工作区手动改过没同步到 git)。
