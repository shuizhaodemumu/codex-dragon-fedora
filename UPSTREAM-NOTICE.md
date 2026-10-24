# 上游来源与授权说明

`renderer.js` 及 `tests/Test-Renderer*.cjs` 来自 HanwenZhang-George/codex-startup-animation 的 community-portable-v34 分支，PR：https://github.com/mytjxs/codex-startup-animation/pull/1 。保留原渲染脚本，不替换作者署名或授予新的许可证。

来源提交：165b187da2762f6fadac942b71905e44f39797c9。原项目来源说明表示未找到上游代码许可证。本包为当前用户本机适配工作，不声明对第三方代码或动画素材获得了新的再分发授权。公开发布前应进一步厘清这些权限。

`src/*.rs` 是本次编写的 Linux 启动器、私有通道宿主和验证程序；`transition.css` 为本次加入的 Linux 泪滴／波纹兼容效果。运行包内素材来自已核验的上游 Release；`startup.mp4` 是本机兼容转码副本。

Rust 依赖：libc、serde_json、base64，具体版本见 Cargo.lock，各依赖保持其各自许可证。依赖完整源码可通过 Cargo 获取。
