# 补位 BuWei

让想来的人，刚好有位。

补位帮助羽毛球、桌游、读书会等社群组织者管理报名、候补和临时补位。
成员使用自己的 Rinx 身份报名和回复，组织者从活动页面查看名额及结果。

作者 **WeiR-h** · 队伍 **生生不息** · Apache-2.0

## 实际效果

创建活动、收集候补、邀请合适成员、核验本人回复。发送中断后，补位沿用原操作编号恢复结果。
AI 帮助理解报名意愿、说明候补结果和准备活动小记。

![活动与候补](assets/screenshots/02-organizer.png)

## 下载

[下载正式 Windows 版本](https://github.com/WeiR-h/buwei/releases/latest)

主分支正在迭代社群活动新版，新增活动卡片、多活动和自动补位。正式下载以 Release 标注的版本为准。

运行环境：Windows 10/11 x64，微软 Visual C++ v14 x64 运行库。
应用基于官方 OctoSense / Rinx 原生宿主，提供组织者和参与者入口。

## 三步开始

1. 解压运行包，打开组织者或参与者入口，在 Rinx 完成本人登录。
2. 打开补位，核对账号并确认所需权限。
3. 组织者创建活动；参与者报名、查看邀请并确认自己的回复。

[使用指南](docs/FIRST-RUN.md) · [隐私说明](docs/PRIVACY.md)

## 源码构建

```powershell
git clone https://github.com/WeiR-h/buwei.git
cd buwei
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1
```

构建需要 Windows x64、Git、Python 3.12+、Rust 1.98.0、Visual Studio C++ 桌面工具和 Windows SDK。
首次构建会获取固定版本的官方依赖。运行与模型配置步骤见使用指南。

核心包含可复用的授权、持久化执行与回执框架，应用源码和必要的构建补丁完整提供。

[English](README.en.md) · [版本记录](CHANGELOG.md) · [第三方声明](NOTICE.md) ·
[问题反馈](https://github.com/WeiR-h/buwei/issues) · [参赛说明](docs/SUBMISSION.md)
