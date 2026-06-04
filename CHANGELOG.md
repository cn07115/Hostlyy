# 更新日志 (Changelog)

本项目的所有重要变更都会记录在此文件中。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

## [1.3.6] - 2026-06-05

### 修复 (Fixed)
- **走代理检查更新报"拉取 latest.json 失败: was redirected to an absolute url with an invalid protocol"**:v1.3.5 修了 URL 拼接漏 `/` 的 11001 DNS bug 后,新错来了。根因:`gh.xmly.dev` / `kkgithub` / `ghproxy` / `ghfast` 见到 `releases/latest/download/latest.json` 都会 **302 跳到** `releases/download/vX.Y.Z/latest.json`,`Location` 头是 `/https://github.com/...` (path-relative)。minreq 默认会 follow redirect,但**错把 path-relative 当 absolute URL 解析**,提取 scheme `https:` + 把 `//github.com/...` 当 host(带 `/` 非法),报"invalid protocol"。修法:`minreq::get(url).with_max_redirects(0)` 禁掉自动 redirect,自己手动 follow(最多 3 次防循环),新加 `resolve_redirect()` helper 处理 absolute / path-relative redirect target。
- **macOS 自动更新从 v1.3.0 起一直没修好**(v1.3.5 修了但没修对):v1.3.6 release `latest.json` 仍然只 windows + linux,缺 darwin 平台。根因:`tauri.conf.json` 的 `productName: "Hostlyy"`(有 y),macOS 实际 build 产出 `.app` bundle 叫 **`Hostlyy.app`**(有 y),但 `build.yml` 9 处全用 `Hostly.app`(无 y)→ `find -name 'Hostly.app'` 永远找不到 → `cp -R` 不执行 → `tar` 跳过 → `signer sign` 跳过 → `add_platform` 跳过 darwin → `latest.json` 没 darwin 节点。跟 v1.3.4 修 Windows installer 时的 typo `Hostly_*` → `Hostlyy_*` 是同一类 bug,只是 v1.3.5 改 path 写法时**没改名字**。修法:全部 `sed s/Hostly\.app/Hostlyy\.app/g`。
- **Windows 开机自启没起作用**:勾了"开机自启" toast 显示成功,但 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 主键里**根本没 Hostlyy 这个 value**。`tauri-plugin-autostart` 2.5.1 在 Windows 上有 bug:它把"已注册"信息写到了 `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run\Hostlyy` 这个**跟踪表**(值是 `02 00 00 ...` = disabled by user),但**没真写到 Run 主键**,所以 Windows 不会启动它。修法:Windows 平台**绕开 plugin**,用 `winreg` crate 直接写 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Hostlyy = "<exe path>" --autostarted`。macOS / Linux 继续用 plugin(那俩平台正常)。
- **托盘菜单 ✓ 标记不实时同步** + **托盘点击没真的切 host**:v1.3.4 加了 `rebuild_tray_menu` 在 active profile label 前打 `✓`,但**只 startup 时调一次**,切 host 后没再调,托盘一直显示启动时的快照。修法:`storage::toggle_profile_active` 完成后调 `crate::rebuild_tray_menu(&app)`,实时同步托盘 ✓。
- **托盘点击只打开编辑器,不切 host**:之前 `tray-select-profile` event handler 调 `selectProfile(id)`(只是把 profile 内容显示到编辑器),不调 `toggle_profile_active`,所以托盘点击不会真的切换当前 host 环境。修法:event handler 改成调 `toggleProfile(id)`(走 multi_select 规则:多选 toggle,单选 设为唯一 active / 再点关掉)+ 同时 `selectProfile(id)` 在编辑器里打开给用户看切换效果。
- **`active_profile_ids` 跟 `profiles[i].active` 不同步**:之前 `toggle_profile_active_internal` 只翻 `profiles[i].active` 标志,从不更新 `active_profile_ids: Vec<String>`,导致 `rebuild_tray_menu` 读 `active_profile_ids` 永远拿不到最新状态(永远是空或者 startup 时的快照)。新加 `sync_active_profile_ids()` helper 在 toggle 后重建 `active_profile_ids`,`set_multi_select_internal` 切换模式后也调一次。

## [1.3.5] - 2026-06-05

### 修复 (Fixed)
- **走代理检查更新永远报"拉取 latest.json 失败: 不知道这样的主机 (os error 11001)"**:Rust 端 `check_update_with_proxy` 拼接 URL 时 `format!("{}...", base)` **漏了 `/` 分隔**,拼出 `https://gh.xmly.devhttps://github.com/...` 这种畸形 URL。minreq 解析时把 `gh.xmly.devhttps://...` 整个当 host,DNS 解析失败 11001。修法: `format!("{}/...", base)` 显式加 `/`。
- **macOS 自动更新从 v1.3.0 起一直不可用**:v1.3.4 release assets 里两个 macOS 平台**都没有 `Hostly.app.tar.gz`**(也没有 `.sig`),所以 `latest.json` 里**没 darwin-aarch64 / darwin-x86_64 平台**,Tauri macOS updater 始终拿不到更新。根因:`build.yml` macOS Stage artifacts 步用写死 path `cp -R src-tauri/target/${{ matrix.target }}/release/bundle/macos/Hostly.app artifacts/`,在 macos-latest M1 runner 上 `host==aarch64-apple-darwin` 时 cargo 会用 `target/release/bundle/...` 而非 `target/aarch64-apple-darwin/release/bundle/...` 写产物,cp 静默失败被 `|| true` 吞掉。修法:改成 `find src-tauri/target -name 'Hostly.app' -type d -exec cp -R {} artifacts/ \;`,匹配 Tauri 2 在两种 layout 下的产物路径(同时打印 `artifacts/` 和 `find` 结果,下次 CI log 直接看到 `.app` 实际位置)。

### 变更 (Changed)
- 默认代理地址 `https://ghfast.top/` → `https://gh.xmly.dev/`(经实测 gh.xmly.dev 在大陆 ISP 通)。
- 加 localStorage 一次性迁移:从老默认 `https://ghproxy.com/` / `https://ghfast.top/` 升级的 user,首次打开设置自动切到新默认(精确字符串匹配,不动用户手动填的值)。
- **清掉 release 里没用的 raw 编译产物**:build.yml 的 Stage artifacts 步不再 copy 3 个 Windows raw `.exe`(`hostly-gui-elevated` / `hostly-cli-elevated` / `hostly-core`,NSIS installer 里已经包含,占 8 MB),也不再 copy 2 个 Linux raw bin(`hostly-bin` / `hostly-core`,因为 softprops `files:` glob 不匹配无扩展名文件,从来没上传成功过,dead code),macOS 的 `hostly-core` 同理也删。Release 体积更小,列表更干净。

## [1.3.4] - 2026-06-04

### 修复 (Fixed)
- **Tauri 自动更新从 v1.3.0 起一直没工作**:`tauri-apps/cli signer sign` 命令**不生成** `latest.json`(它只产 `.sig` 文件),build.yml 之前依赖这一步产 latest.json 是错的。手动拼 latest.json(收集所有 `.sig` + 对应 installer URL,生成完整 manifest),同时修 Sign installers step 的 file pattern 从 `Hostly_*` 改 `Hostlyy_*`(有 y 才是产品名,旧 pattern 找不到 Windows installer,Windows .sig 一直缺)。装 v1.3.4 之后 `updater.check()` 能拉到 valid JSON,自动更新链路打通。
- **托盘右键子菜单"已选择"无标记**:`rebuild_tray_menu` 读 `active_profile_ids`,active profile 的 label 前加 `✓ ` 前缀(在子菜单里能看到当前启用的环境)。

### 变更 (Changed)
- 默认代理地址 `https://ghproxy.com/` → `https://ghfast.top/`(ghproxy.com 在很多 ISP 已 TCP 不通,ghfast.top 走 Cloudflare 较稳)。
- **修 macOS updater tar.gz 步骤**:v1.3.3 release 缺 `Hostly.app.tar.gz`(原 `artifacts/**/Hostly.app` glob 没匹配到),Tauri macOS updater 一直不可用。改为显式遍历多个常见路径 + 加 `set -x` + 打印 artifacts/ 目录结构,下次 CI log 能看到 .app 真实位置。

## [1.3.3] - 2026-06-04

### 修复 (Fixed)
- **托盘右键子菜单"快捷选择 hosts"不显示**:v1.3.0 加的托盘子菜单在所有已发布版本都没出现,根因是 `TrayIconBuilder::new()` 用 `TrayIconId::new_unique()` 生成随机 id,后续 `app.tray_by_id("main")` 永远拿不到,`rebuild_tray_menu` 直接 `return`,菜单没被替换,用户看到的还是 TrayIconBuilder 初始设的 `show + quit` 两项。改为 `TrayIconBuilder::with_id("main")` 固定 id,`tray_by_id` 才能取到。

## [1.3.2] - 2026-06-04

### 新增 (Added)
- **检查更新支持 ghproxy 代理**:"关于"页「检查更新」按钮上方新增复选框 + 代理地址输入框(默认 `https://ghproxy.com/`)。勾选后,后端用 `minreq` 拉 `latest.json` 并 sed 替换当前 OS 对应 platform 的 `url` 字段走代理;发现新版本会直接调系统默认浏览器/下载工具打开拼接好的下载链接,避免中国网络下 `error sending request for url latest.json`。**启动检查**和**手动检查**共用同一套代理设置(localStorage 持久化,首次安装未改过时默认直连,不打扰海外用户)。不勾选时维持原 `tauri-plugin-updater` 直连 GitHub 的行为。

## [1.3.1] - 2026-06-04

### 修复 (Fixed)
- **updater 检查更新被 ACL 拒绝**:`Command plugin::updater|check not allowed by ACL`,在 `capabilities/default.json` 加 `updater:default` 授权
- **窗口标题 `{{version}}` 不替换**:Tauri 2 不展开 `tauri.conf.json` 标题里的 `{{version}}` 模板,改为运行时 `window.set_title("Hostlyy v" + version)` 动态设置,跟版本号走

---

## 历史变更汇总 (v1.0.0 ~ v1.3.0)

本节整合 v1.0.0 初始发布到 v1.3.0 期间所有面向用户的功能变更。早期 GitHub release 页面保留作为历史快照。

### v1.3.0 - 2026-06-04

#### 新增 (Added)
- **应用内自动更新**:接入官方 `tauri-plugin-updater`,关于 → 检查更新一键检测 + 下载安装,签名校验
- **启动自动检查更新**:启动 3 秒后异步 check,有新版弹窗显示版本号 + 更新日志
- **托盘快捷选择 hosts**:系统托盘右键子菜单,列全部 profile 切换
- **关于页**:显示应用名 / 版本 / GitHub / 检查更新
- **设置面板重新排版**:左栏「通用 / 同步 / 关于」三栏,WebDAV 独立到「同步」

#### 变更 (Changed)
- 应用名 `Hostly` → `Hostlyy`(窗口标题、托盘、sidebar、release 名)
- 窗口标题增加版本号

#### 修复 (Fixed)
- **「跟随系统」模式不跟 OS 切换**:WebView2 上 `onThemeChanged` 偶尔不触发,加 `matchMedia('change')` 兜底

### v1.2.8 - 2026-06-04
（无功能变更）— CHANGELOG 文档重构

### v1.2.7 - 2026-06-04
- **WebDAV 内容去重(SHA-256)**:本地 profile 未实际改动不再重传
- **主题「跟随系统」**:明亮 / 深色 / 系统 三选一
- 浅色主题 CSS 颜色硬编码修复

### v1.2.6 - 2026-06-04
- **WebDAV 同步并行化**:多文件并发,延迟 N×RTT → ~1×RTT
- 首次同步「30 天未更新」误报修复
- 同步完成 toast 简化

### v1.2.5 - 2026-06-03
- **WebDAV 文件路径错位修复**:文件改写到 `hostly/profiles/` 下

### v1.2.4 - 2026-06-03
- **「未配置」状态显示误导修复**

### v1.2.3 - 2026-06-03
- **测试连接删除 keychain 密码修复**
- 同步按钮 TypeError 修复
- 测试连接未配置提示优化

### v1.2.2 - 2026-06-03
- **WebDAV 错误事件**:后台同步失败 toast
- **未配置静默跳过**
- URL 打开不再闪 cmd 窗口
- 设置面板标题间距调整

### v1.2.1 - 2026-06-03
- **WebDAV 自动同步**:5s 防抖 + 启动拉取 + 5min 周期拉取

### v1.2.0 - 2026-06-03
- **WebDAV 多设备同步**
- **凭证存 keychain**(Windows Credential Manager / macOS Keychain / Linux Secret Service)
- **存储拆分**:`config.json` → `config.local.json` + `config.sync.json`

### v1.1.0 - 2026-06-03
- **跨平台自动提权**(Windows runas / macOS osascript / Linux pkexec)
- **跨平台开机自启**(tauri-plugin-autostart)
- **关闭按钮 in-app 确认弹窗**(退出 / 最小化到托盘 / 记住选择)
- **托盘图标功能**(Windows / macOS / Linux)

### v1.0.0
- 初始发布:hosts 编辑、系统 Hosts(只读) / 公共配置切换、单选 / 多选模式、导入(SwitchHosts) / 导出

