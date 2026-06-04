# 更新日志 (Changelog)

本项目的所有重要变更都会记录在此文件中。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

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

