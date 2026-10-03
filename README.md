# 补位 BuWei

让想来的人，刚好有位。

作者 **WeiR-h** · 队伍 **生生不息** · Apache-2.0

补位是运行于官方 OctoSense / Rinx 宿主中的 Windows 原生扩展。
组织者管理一场活动，参与者用自己的 Rinx 身份报名、接受、拒绝或取消。
模型帮助理解需求，席位由候补顺序、时段和容量规则决定。
邀请和文章在确切预览后由本人确认；服务端事件核实后才显示成功。

**当前源码：v0.1.1 小版本，验收中。**
新增多轮需求追问和 62 项合成模型测试，见 [模型辅助与测试](docs/MODEL-EVAL.md)。
**已冻结稳定版：v0.1.0。** [下载运行包与材料](https://github.com/WeiR-h/buwei/releases)。
运行包需要 Windows 10/11 x64 和最新微软 Visual C++ v14 x64 运行库，
官方下载入口见 [首次运行](docs/FIRST-RUN.md)。无需 Rust 或 Visual Studio。
85 项 Rust、7 项发布工具检查、五轮真实双账号、拒绝、过期及三类故障
验收通过。独立 Windows 构建和另一台全新 Windows 运行环境均通过。
稳定标签须通过公开下载复验；详细结果见 [验收记录](docs/ACCEPTANCE.md)。

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
- 真实恢复、原文章编辑器和干净环境结果见 [验收记录](docs/ACCEPTANCE.md)。

[English](README.en.md) · [隐私](docs/PRIVACY.md) · [版本记录](CHANGELOG.md) ·
[第三方声明](NOTICE.md) · [支持](https://github.com/WeiR-h/buwei/issues)

运行包来自干净 Windows CI，源码和二进制构建提交分别记录在 PROVENANCE.json。
文档更新不会改变已验证的原生源码、依赖和可执行文件。正式附件经隐私扫描，
不包含登录、模型密钥或业务数据。支持渠道为仓库 Issues。
