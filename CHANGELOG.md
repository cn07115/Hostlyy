# 更新日志 (Changelog)

本项目的所有重要变更都会记录在此文件中。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

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

