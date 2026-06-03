# 更新日志 (Changelog)

## [Unreleased]

## [1.2.6] - 2026-06-04

### 🐛 修复 + 性能 (Fixes & Performance)

- **修 toast 啰嗦**:`webdavSyncBtn` 之前会拼出「同步完成: 上传 10 个 | ⚠ hostly/profiles/<id>.txt: 本地修改时间比远端新 20607 天; ...」一长串。**修法**:toast 只显示「同步完成」+ 极简统计(上传 N 个 / 下载 N 个 / 删除 N 个),warning 字符串不再进 toast(只在 console.warn 留个 log);错误时 toast 只报「部分完成 (X 个错误)」,详细错误进 console.error。完整 SyncResult 一直在 console 里,要查细节 F12 开 devtools 就能看。
- **修 staleness 检查 false positive**:`check_staleness` 之前对所有「remote_mtime == 0」的文件都会警告,因为 0 视作 epoch,而本地 mtime 是「现在」,差值就是 20607 天(epoch 到 2026)。**修法**:`remote_mtime == 0` 时直接返回 None,不警告。**注意**:这一改也意味着**首次 sync 不再 toast 一堆警告**(真警告只有后续 sync 中"远端之前更新过、本地又改"的情况)。
- **修 sync 慢**:`perform_sync` 5a/5b/5c 三个循环之前是**单线程串行** HTTP,10 个 profile 文件要 10*RTT 才能传完。**修法**:用 `std::thread::scope` 把所有 PUT/GET/DELETE 并行起来,4 个结果收集器各自一个 `Arc<Mutex<Vec>>`,每个文件独立线程跑。理论上 N 个文件从 N*RTT 降到 ~1*RTT(实际受 WebDAV server 并发连接数限制,通常 5-10 倍加速)。**注意**:每个 HTTP 调用还是用 minreq sync 模式(没共享 Client 连接池),所以实际加速比理论小,后续可以再优化成共享 Client。

## [1.2.5] - 2026-06-03

### 🐛 修复 (Fixes)

- **修 WebDAV 文件夹结构 bug(文件全错位到根目录)**:`perform_sync` 里 MKCOL 用了 `REMOTE_DIR` + `PROFILES_REMOTE_DIR` 常量(`<base>/hostly` + `<base>/hostly/profiles`),但 PUT/GET 文件时却**没用**,直接拼 `<base>/config.sync.json` 和 `<base>/profiles/<id>.txt`,文件全落到根目录。**修法**:所有 PUT/GET URL 都改成用常量拼(`{base}/{REMOTE_DIR}/config.sync.json` 和 `{base}/{PROFILES_REMOTE_DIR}/{id}.txt`),filter `url.contains("/profiles/")` 同步改成 `/hostly/profiles/`,display name 也改成 `hostly/...` 前缀。**注意**:旧版本已经写到根目录的 orphan 文件需要手动 WebDAV 客户端清理(`config.sync.json` + `profiles/<id>.txt` 在根目录的),新版本只往 `hostly/` 下面写。

## [1.2.4] - 2026-06-03

### 🐛 修复 (Fixes)

- **修「保存配置后状态反而显示 未配置」**:后端 `save_webdav_config` 每次保存都会清空 `webdav_last_status = None`(line 238,注释「Clear status on config change」),本意是配置改了,旧状态失效。但前端 `formatSyncStatus` 把 `last_status == null` 解读为"未配置",就把"刚保存还没同步"显示成了"没配置",误导用户以为 keychain/网络又出问题了。**修法**:前端 `formatSyncStatus` 改吃整个 `SyncStatus` 对象,用后端已经返回的 `s.configured` 字段(`webdav_url.is_some() && webdav_username.is_some()`)判断"未配置";`last_status == null` 改为显示「已配置,未同步」,跟「未配置」清晰区分。

## [1.2.3] - 2026-06-03

### 🐛 修复 (Fixes)

- **修「点击测试连接后密码被删」的连锁 bug**:测试连接按钮原本会先 `save_webdav_config(url, username, password)` 再测试,但密码框在「保存配置」成功后会被**故意清空**(防泄漏),导致测试按钮读到空 password → 后端 `save_credentials` 走"空密码=删 entry"分支 → keychain 里的密码被清掉 → 紧接着 `test_webdav_connection` 报"读取 keychain 失败: No matching entry"。**修法**:测试按钮**不再 auto-save**,直接测已保存的配置;如果还没保存,后端返回友好提示「请先填写并点击 保存配置」。
- **修 `TypeError: t.summary is not a function`**:Rust 的 `SyncResult::summary()` 是方法,Tauri IPC 跨进程序列化时**只带 struct 字段,方法全丢**。前端同步按钮原本调 `result.summary()`,所以同步成功但 toast 报红。**修法**:前端 `main.js` 加 `formatSyncSummary(r)` helper 复算一遍(同样的上传/下载/远端删除/错误 计数逻辑)。
- **后端 `test_webdav_connection` 错误信息更友好**:未配置时返回「请先填写并点击 保存配置」(之前是干巴巴的「WebDAV URL 未配置」),引导用户走正确的保存流程。

## [1.2.2] - 2026-06-03

### 🔄 同步体验 (Sync UX)
- **后台错误可见**: 自动同步 / 启动拉取 / 周期拉取 失败时(配置错误、网络断开、凭证失效)会发 `webdav-error` 事件,前端用 toast 提示用户,不再 `eprintln!` 静默吞错
- **未配置静默跳过**: `sync_now_internal` 改返回 `Result<Option<SyncResult>, String>`,未配置返回 `Ok(None)`,前端同步按钮显示 info toast "WebDAV 未配置,跳过了同步",**不再弹"未配置"红色错误**
- **凭证错误分类**: 之前所有错误都堆在一起,现在 keychain 凭证缺失会单独返回"凭证读取失败: ..."(并在 status 里写 `error: credentials: ...`),方便定位

### 🐛 修复 (Fixes)
- **软件内点 URL 不再闪 cmd 窗口**: `hostly_open_url` 从 `cmd /C start <url>` 换成 `open::that_detached`,跨平台统一,且 Windows 不再临时分配控制台
- **优化设置页标题与下方控件间距**: `.pane-title` 边距从 `28px` 调到 `20px`,padding-bottom 改成 `14px`,再加 `.pane-title + .form-group { margin-top: 16px }`,分隔线和第一个 form-group 之间有合理呼吸空间(之前要么贴太近要么空太大)

## [1.2.1] - 2026-06-03

### 🔄 自动同步 (Auto-sync)
- **防抖 5s 同步**:每次本地改动(创建/删除/重命名/切换激活/多选/更新 remote config)自动调度一次同步,5 秒内无新改动才真正推送。批量合并,避免高频小请求。
- **启动拉取**:应用启动 3 秒后自动 pull 一次,确保打开时本地就是最新。
- **周期拉取**:后台每 5 分钟自动 pull 一次,兜底多设备同步(防止有设备长期不在线错过更新)。
- **30 天警告**:本地修改时间比远端新超过 30 天时,推送前会带上警告提示(防止离线过久覆盖别人近期改动),toast 会显示。

## [1.2.0] - 2026-06-03

### 🚀 新增功能 (Features)
- **WebDAV 多设备同步**
  - 设置页加 "云端同步 (WebDAV)" 区域,填 URL + 用户名 + 密码即可启用
  - 支持多设备间同步 hosts 配置(注意:系统设置本身不同步,只同步 `config.sync.json` + `profiles/*.txt`)
  - 冲突策略: **last-write-wins**(基于 HTTP Last-Modified 头)
  - 凭证安全: 密码存**系统 keychain**(Windows Credential Manager / macOS Keychain / Linux Secret Service),不写 `config.local.json`
  - 一键 "测试连接" 验证 URL + 凭证
  - 一键 "立即同步" 双向同步,toast 显示上传/下载/删除统计

### 🔧 架构调整 (Refactor)
- **存储拆分**
  - `config.json` 拆成 `config.local.json`(系统设置)+ `config.sync.json`(hosts 元数据)
  - 系统设置:theme / window_*/sidebar_width / auto_start / close_behavior / remember_close_choice / webdav 配置
  - 同步数据:multi_select / profiles / active_profile_ids
  - 启动时**自动迁移**老的 `config.json`(检测到就拆,旧文件保留作为备份)
  - `AppConfig` 仍作为合并视图返回给前端(UI 不需要改)
- 加 `keyring = "3"` crate(跨平台系统 keychain 抽象)
- WebDAV HTTP 复用现成 `minreq`,PROPFIND / MKCOL / PUT / GET / DELETE 用 `Method::Custom` 实现

### 🔄 自动同步 (Auto-sync)
- **防抖 5s**:每次本地改动(创建/删除/重命名/切换激活/多选/更新 remote config)自动调度一次同步,5 秒内无新改动才真正推送。批量合并,避免高频小请求。
- **启动拉取**:应用启动 3 秒后自动 pull 一次,确保打开时本地就是最新。
- **周期拉取**:后台每 5 分钟自动 pull 一次,兜底多设备同步(防止有设备长期不在线错过更新)。
- **30 天警告**:本地修改时间比远端新超过 30 天时,推送前会带上警告提示(防止离线过久覆盖别人近期改动),toast 会显示。

### 🐛 修复 (Fixes)
- 升级 `tauri-plugin-autostart` 到 2.5.1(旧版本在 macOS 上偶发不生效)
- 修复设置页 `.pane-title` 标题与其下方第一个 form-group 间距过小(从 20px → 28px + 10px 内边距)
- 修复软件内 GitHub 链接指向不存在的 `zengyufei/Hostlyy`(拼写错误,应小写 h),改为新 fork `cn07115/Hostlyy`

## [1.1.0] - 2026-06-03

### 🚀 新增功能 (Features)
- **跨平台 Auto-Elevation (提权)**
  - Windows: 进程启动时检查 `IsUserAnAdmin`,非管理员自动用 `ShellExecuteExW` + `runas` 提权重启(**原 `net session` 子进程方案被替换,启动时不再有 cmd 窗口一闪而过**)。
  - macOS: `osascript -e 'do shell script ... with administrator privileges'` 触发系统原生认证框。
  - Linux: 优先 `pkexec`(走 polkit 弹 GTK/Qt 认证框),fallback 到 `sudo`。
- **跨平台 Auto-Start (开机自启)**
  - 改用 `tauri-plugin-autostart` 替代原作者手写的 Windows 注册表实现。
  - Windows: HKCU 注册表 / macOS: `~/Library/LaunchAgents` plist / Linux: `~/.config/autostart` .desktop。
  - 原作者的 `autostart.rs` 只 Windows 有效,现在三平台都跑。
- **关闭按钮修复**
  - 之前系统 `ask()` 弹窗在 Tauri 2 上会静默失败,自定义弹窗又因 CSS 缺 `#close-confirm-overlay` 选择器不显示,体感"关闭按钮无反应"。
  - 现在点 X 直接显示应用内居中弹窗(带"退出程序 / 最小化到托盘 / 记住本次选择"),不再依赖系统弹窗。
- **GitHub Actions 多平台 CI**
  - 新增 `.github/workflows/build.yml`:Windows (msvc) + macOS (arm64 + x64) + Linux 矩阵并行构建,产物作为 Artifacts 上传 14 天。
  - 删除原作者 `.github/workflows/publish.yml`(每次 push 删旧 release,对 fork 没意义)。

### 🐛 修复 (Fixes)
- 启动时 cmd 窗口一闪而过 —— 根因是 `relaunch_as_admin_if_needed` 调了 `net session` 子进程,从 `windows_subsystem` 父进程 spawn 控制台子系统子进程会临时分配控制台。
- 关闭按钮"无反应" —— 根因是事件监听走 `tauri-plugin-dialog::ask()` 系统弹窗失败,fallback 到 `showCloseDialog()` 自定义弹窗但 CSS 缺 `.modal-overlay` 选择器导致不显示。
- macOS / Linux 自启动按钮无效 —— 原 `autostart.rs` 只 Windows 工作(`#[cfg(target_os = "windows")]`),其它平台返回 `false`。
- 补回 `tray-icon` feature 和 `tauri-plugin-autostart` 依赖 —— 原作者 `Cargo.toml` 漏了,README 吹了有托盘但 feature 没开,本地工作区手动修过但 git push 时漏同步。

## [Unreleased] - 2026-01-14

### ✨ 新增功能 (Features)
- **远程配置支持 (Remote Config)**
  - 支持导入远程 URL 作为 Hosts 配置源。
  - 实现自动更新机制（默认 1 小时检查一次）。
  - 后端使用 `minreq` 库处理 HTTP 请求。
  - 前端增加“最后更新时间”和“下次更新时间”状态显示。
- **配置导出 (Export)**
  - 支持将当前配置（系统/本地/远程）导出为文本文件。
  - 交互优化：点击顶部标题栏即可触发导出。
- **全局配置持久化 (Global Persistence)**
  - 用户的主题设置、窗口大小、侧边栏宽度等配置现已保存至 `config.json`。
  - 实现后端命令 `set_theme`, `save_window_config`, `save_sidebar_config`。

### 💄 界面与体验优化 (UI/UX)
- **主题切换 (Theming)**
  - 完善的明亮/深色模式支持。
  - **极致防闪烁优化**：
    - 程序启动时隐藏窗口，待内容渲染完毕后显示。
    - 引入动态启动脚本，根据保存的配置或系统偏好预设背景色，彻底消除“白屏/黑屏”闪烁。
- **窗口管理 (Window Management)**
  - **侧边栏拖拽**：支持拖拽调整左侧边栏宽度 (200px - 600px) 并自动保存。
  - **启动大小记忆**：新增“窗口设置”，可选固定比例 (如 1024x768) 或“记住上次退出时的大小”。
- **细节打磨**
  - 重构刷新按钮位置，整合至列表项中。
  - 优化弹窗 (Modal) 样式与交互。

### 🐛 修复 (Fixes)
- 修复了 `main.js` 中的部分语法错误。
- 修复了明亮模式下编辑器字体颜色过浅的问题（改为纯黑）。
- 解决了启动时由于窗口尺寸调整导致的视觉抖动问题。
