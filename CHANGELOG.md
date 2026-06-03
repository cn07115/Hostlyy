# 更新日志 (Changelog)

本项目的所有重要变更都会记录在此文件中。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/),
版本遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

## [1.3.0] - 2026-06-04

### 新增 (Added)
- **应用内自动更新**:接入官方 `tauri-plugin-updater`,设置 → 关于 → 检查更新按钮一键检测 + 下载安装新版本,签名校验防中间人替换
- **托盘快捷选择 hosts**:系统托盘右键新增「快捷选择 hosts」子菜单,列出全部 profile,点击直接切换到该 profile(等同在主界面双击)
- **关于页**:设置新增「关于」标签,显示应用名 / 当前版本 / GitHub 仓库地址(含复制按钮) / 检查更新按钮
- **设置面板重新排版**:左侧导航分为「通用 / 同步 / 关于」三栏,WebDAV 同步从通用里独立出来,放在「同步」一栏

### 变更 (Changed)
- 应用名 `Hostly` → `Hostlyy`(窗口标题、托盘 tooltip、sidebar 标题、关于页、GitHub release 标题)
- 窗口标题增加版本号显示:`Hostlyy v1.3.0`(用 `{{version}}` 模板,bump 版本时自动同步)

### 修复 (Fixed)
- **「跟随系统」模式不跟随 OS 切换**:WebView2 上 `WebviewWindow.onThemeChanged` 偶尔不触发,加 `matchMedia('(prefers-color-scheme: light)').addEventListener('change', ...)` 兜底,任何 webview 都能跟 OS 实时切深浅

## [1.2.8] - 2026-06-04

（无功能变更）

## [1.2.7] - 2026-06-04

### 新增 (Added)
- **WebDAV 内容去重(SHA-256)**:本地 profile 文件未实际改动时,不再重传(修了 mtime 精度差导致每次同步都重传同样内容的 bug)
- **主题「跟随系统」**:明亮/深色之外加第三选项,跟随 OS 实时切换

### 修复 (Fixed)
- 浅色主题 CSS 颜色硬编码修复(`.type-switch` 背景 / `.footer-text-btn:hover` 边框)

## [1.2.6] - 2026-06-04

### 性能 (Performance)
- **WebDAV 同步并行化**:多文件并发上传/下载/删除,从 N×RTT 降到 ~1×RTT(受 WebDAV 服务端并发限制)

### 修复 (Fixed)
- 首次同步「30 天未更新」误报修复
- 同步完成 toast 简化,详情改 F12 devtools 查看

## [1.2.5] - 2026-06-03

### 修复 (Fixed)
- **WebDAV 文件路径错位**:文件原落到 WebDAV 根目录,现改写到 `hostly/profiles/` 下(**注意**:旧版本写到根目录的 orphan 文件需手动清理)

## [1.2.4] - 2026-06-03

### 修复 (Fixed)
- **「未配置」状态显示误导**:「已配置未同步」与「未配置」分开显示

## [1.2.3] - 2026-06-03

### 修复 (Fixed)
- **测试连接删除 keychain 密码**:测试按钮不再自动保存,先点保存再测试
- 同步按钮 TypeError 修复
- 测试连接未配置提示优化

## [1.2.2] - 2026-06-03

### 新增 (Added)
- **WebDAV 错误事件**:后台同步失败时 toast 提示
- **未配置静默跳过**:手动同步按钮显示 info toast 而非报错

### 修复 (Fixed)
- URL 打开不再闪 cmd 窗口
- 设置面板标题间距调整

## [1.2.1] - 2026-06-03

### 新增 (Added)
- **WebDAV 自动同步**:5 秒防抖合并 + 启动拉取 + 5 分钟周期拉取

## [1.2.0] - 2026-06-03

### 新增 (Added)
- **WebDAV 多设备同步**:设置页加 WebDAV 面板,多设备同步 hosts 配置,凭证存系统 keychain
- **存储拆分**:`config.json` 拆成「系统设置」+「同步数据」,自动迁移老 config

## [1.1.0] - 2026-06-03

### 新增 (Added)
- **跨平台自动提权**:Windows / macOS / Linux 提权方案统一
- **跨平台开机自启**:三平台统一自启方案
- **关闭按钮 in-app 确认弹窗**:自定义 modal,提供「退出 / 最小化到托盘 / 记住选择」
- **托盘图标功能**:Windows / macOS / Linux 三平台支持

### 修复 (Fixed)
- 启动时 cmd 窗口一闪而过修复
- macOS/Linux 自启动按钮无效修复

