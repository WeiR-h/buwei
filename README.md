# 补位 BuWei

让想来的人，刚好有位。

补位帮助羽毛球、桌游、读书会等社群组织者持续跟进报名、候补和临时补位。
组织者保存想办成的活动，成员保存想参加的安排；补位发现相关变化、准备下一步，并沿原操作编号核实结果。

作者 **WeiR-h** · 队伍 **生生不息** · Apache-2.0

## 实际效果

从创建活动、分享卡片，到报名、候补和本人接受，在一个活动页面内完成。

- 同时管理最多五场活动，每场最多 30 个名额；支持完整日期、分钟和跨午夜时段。
- 同行 1–8 人整组报名。名额不足时保留排位，人数合适时再邀请。
- 确认本场规则后自动补位，分别显示名额保留、邀请送达和本人接受。
- “我的目标”保存本次安排和本人确认的长期偏好，主动提示人数缺口、合适活动、待回复邀请及时间冲突。
- 建议衔接持续任务；修改或暂停目标后，旧预览停止执行，已有回执保留用于恢复。
- AI 帮助生成活动草稿、理解报名、解释候补和准备有事实依据的活动小记。
- 发送中断后沿原操作编号核实结果，避免重复邀请或发布。

![活动与候补](assets/screenshots/02-organizer.png)

## 下载

[下载正式 Windows 版本](https://github.com/WeiR-h/buwei/releases/latest)

当前源码为 **v0.2.3 发布候选**，正式下载以 Release 标注的版本为准。

应用使用固定版本的官方 OctoSense 与 Rinx 1.1.0，提供组织者和参与者入口。成员使用自己的 Rinx 身份报名和回复，组织者核验结果后显示最终报名状态。

用户可主动开启 Windows 托盘值守；电脑与程序运行且授权有效时继续同步。反馈只修改本人选择的范围，下一场筹备生成可编辑草稿，日期、地点和人数重新确认。

[目标、主动建议与值守](docs/INTENTIONS.md) · [参赛与历史版本](docs/SUBMISSION.md)

三分钟操作讲解随对应 Release 提供，使用人工审阅的实际原生界面截图序列。

运行环境：Windows 10/11 x64，微软 Visual C++ v14 x64 运行库。
发布形式为 Windows 原生宿主扩展。

## 三步开始

1. 解压运行包，打开组织者或参与者入口，在 Rinx 完成本人登录。
2. 打开补位，核对账号并确认所需权限。
3. 保存本人目标：组织者创建活动并分享卡片；参与者从卡片进入报名，收到主动建议或邀请后核对并确认。

[使用指南](docs/FIRST-RUN.md) · [隐私说明](docs/PRIVACY.md)

## 源码构建

```powershell
git clone https://github.com/WeiR-h/buwei.git
cd buwei
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1 -Release
```

构建需要 Windows x64、Git、Python 3.12+、Rust 1.98.0、Visual Studio C++ 桌面工具和 Windows SDK。
首次构建会获取固定版本的官方依赖。运行与模型配置步骤见使用指南。

核心包含可复用的授权、持久化执行与回执框架，应用源码和必要的构建补丁完整提供。

[English](README.en.md) · [版本记录](CHANGELOG.md) · [第三方声明](NOTICE.md) ·
[问题反馈](https://github.com/WeiR-h/buwei/issues) · [参赛说明](docs/SUBMISSION.md)
