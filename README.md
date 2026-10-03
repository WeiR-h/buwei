# 补位 BuWei

让想来的人，刚好有位。

作者 **WeiR-h** · 队伍 **生生不息** · Apache-2.0

补位是运行于官方 OctoSense / Rinx 宿主中的 Windows 原生扩展。
组织者管理一场活动，参与者用自己的 Rinx 身份报名、接受、拒绝或取消。
模型帮助理解需求，席位由候补顺序、时段和容量规则决定。
邀请和文章在确切预览后由本人确认；服务端事件核实后才显示成功。

**当前源码：v0.1.0 发布候选，稳定附件尚未公开。**
五轮真实双账号、拒绝、过期及三类故障验收已通过，完整运行包验收继续进行。
核心测试、真实服务器和干净 Windows 环境分别记录。

## 源码构建

需要 Windows x64、Git、Python 3.12+ 和 Rust 1.98.0。公开运行包采用
MSVC，安装 Visual Studio 的“使用 C++ 的桌面开发”和 Windows SDK。
Visual Studio / MSVC 工具版本记录在构建来源中。GNU / MinGW 另有本机
核心检查与真实 SDK 验证；其 GCC / Binutils 版本也在构建来源中记录。
首次构建需要网络下载固定依赖，不需要模型密钥或 Rinx 登录。

```powershell
git clone https://github.com/WeiR-h/buwei.git
cd buwei
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1
```

构建完成后运行 `native/target/debug/buwei-rinx-dual-host.exe`。
独立资料目录及 `--gui` 入口见 [首次运行](docs/FIRST-RUN.md)。
不要把资料目录放入源码仓库。

`native/` 包含扩展和两个 Rust 核心库。`.deps/` 是被忽略的官方依赖目录。
依赖补丁、工具链和固定提交全部列明，无需相邻旧工程。

## 范围

- Windows，一场活动，最多 30 人，一个组织者宿主。
- SDK 身份、授权、摘要、业务版本和有效期共同绑定确认。
- 发送前保存操作；不明结果保留原编号，核实恢复，禁止盲目重发。
- 报名、回复、取消、邀请与文章共用 `action-receipts`。
- MiniMax 通过官方模型宿主调用，未配置时可手动使用。
- 真实恢复、原文章编辑器和干净环境按 [验收记录](docs/ACCEPTANCE.md) 推进。

[English](README.en.md) · [隐私](docs/PRIVACY.md) · [版本记录](CHANGELOG.md) ·
[第三方声明](NOTICE.md) · [支持](https://github.com/WeiR-h/buwei/issues)

运行包须在全新 Windows runner 构建和启动，检查结果见验收记录。
本机调试包仍包含编译路径，仅用于私有测试；正式附件须通过运行包扫描。
