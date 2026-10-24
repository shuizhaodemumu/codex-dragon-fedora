# v0.2 验证记录 · 2026-10-24

环境：Fedora 44 / niri / Wayland，Rust 1.99.1，Chrome 156，Linux 客户端内核 Chrome 155。

## v0.2 已验证

- 独立配置启动的 Linux 客户端通过继承的私有通道响应 Browser.getVersion，能够发现 app://-/index.html 页面；未开放 TCP 端口。
- Rust 单元测试 7 项：命令与 URI、目标 URL 约束、内存恢复滞回策略。
- 启动流程集成测试 8 项：沿用 v0.1 的成功、失败、超时、并发和 Wayland 后端测试。
- 原 renderer.js 布局回归 166 项通过，内存行为回归 22 项通过，质量指标边界检查 8 项通过。
- 独立 Chrome、本地模拟应用页面和原始视频的完整自测通过：首帧确认、播放时钟推进、粒子解码、透明主区与输入区、控件点击、泪滴／波纹样式、清理恢复、重新安装渲染器。
- 实际 Rust 宿主控制测试通过：本地 socket 状态查询、关闭后的 cleanupConfirmed、再次启用、媒体错误后恢复、页面重载后重新接入、最终关闭清理。
- 两个壁纸视频与原媒体清单逐项 SHA256 核对通过。没有重新编码两个壁纸视频。
- 从当前 Linux 客户端的只读 ASAR 样式资源确认 MainContentSurface、WorkspaceContent、ComposerLayoutRoot 及相关语义属性仍存在；这属于静态兼容依据，不代替运行时验证。
- 本机已升级到 v0.2，菜单文件校验通过；安装后的程序及全部已跟踪文件与安装清单哈希一致。当前旧会话正确报告尚无壁纸宿主，需要下次从新入口启动。
- 泪滴／波纹为 Linux 新增兼容效果，截图来自本地测试页面；不声称原版 Windows 窗口交接已逐帧复现。

## 真实账号验证边界

当前用户正在使用的 Codex 未被关闭，也未被注入。上述浏览器功能验证使用本地模拟工作区，不能代替真实账号界面的冷启动验收。

安装后，保存工作、正常完整退出 Codex，再从“Codex 龙娘启动动画”打开。`--wallpaper-status` 中同时出现 playing、firstFrameConfirmed、wallpaperApplied 为 true 才能确认当前真实页面已播放；只有程序启动或通道连接不能视作完成播放验证。

后续应用更换页面结构、CSP 或运行时仍可能使适配失效。目标文档或布局不匹配时停止接入，视频错误恢复失败后停用并尝试恢复样式；关闭操作是否确认清理由 cleanupConfirmed 明确报告。

---

以下保留 v0.1 启动动画版的历史记录；其“尚未实现”条目仅描述 v0.1。

# v0.1 历史验证记录

环境：Fedora 44 x86_64，niri / Wayland，屏幕 2560×1440 / 60Hz，Rust 1.99.1，FFmpeg 8.1.3。

## 通过的检查

- 原 Release ZIP 大小为 369241850 字节，SHA256 与项目发布值相符。
- 提取的原启动视频大小、SHA256 与源码媒体清单相符。
- Rust 单元测试 3 项通过：参数和 URI 保留、无效选项拒绝、指定预览素材路径。
- 集成测试 8 项通过：参数原样传递、并发互斥、失败预览返回错误、卡住 20 秒后的恢复、缺失素材后的恢复、播放器失败后的恢复、预览不启动应用、Wayland 原生 SDL 后端。
- release 构建成功，产物为 x86_64 Linux ELF，约 360 KiB。
- 安装、卸载、重新安装通过。原 Codex 桌面入口未覆盖。
- 独立 desktop 文件通过 desktop-file-validate。
- 已安装程序 `--check` 返回成功，能识别正在运行的 Codex。
- 已安装程序实际预览正常退出，耗时 8.98 秒；niri 观察到标题为 Codex Dragon 的窗口，app_id=ffplay，获得焦点。
- 实际播放结束后启动指定测试子程序的交接验证通过，未重启正在使用的 Codex。

## 发现并处理的问题

1. 原 144fps H.264 在当前默认解码器下播放未在 20 秒内完成。保留原文件，另生成 1440p60 MPEG-4 Part 2 副本。
2. SDL 自动选择显示后端时，未在 niri IPC 窗口列表观察到播放窗口；指定原生 Wayland 后正常观察到窗口和焦点。启动器现在默认选择当前 Wayland 会话。

这些检查验证了文件、程序行为和窗口存在性，没有逐帧检查主观画质。ESC 按键遵循 ffplay 默认控制，未做自动键盘注入测试。

## 尚未验证或实现

- 当前会话仍在使用 Codex，因此没有关闭并重新启动真实 Codex；冷启动后的焦点与窗口排列仍需实际使用确认。
- 应用内动态壁纸、透明输入框、粒子壁纸和 Windows 原版波纹交接尚未实现。
- 没有修改 Codex 应用文件、原桌面入口、默认协议处理器或系统级软件包。

## 安装位置

- 程序：`~/.local/bin/codex-dragon`
- 运行素材及卸载脚本：`~/.local/share/codex-dragon/`
- 新入口：`~/.local/share/applications/codex-dragon.desktop`

预览：`~/.local/bin/codex-dragon --preview`

卸载：`python3 ~/.local/share/codex-dragon/install.py --uninstall`
