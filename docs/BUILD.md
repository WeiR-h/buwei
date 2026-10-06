# 从源码构建

Windows x64，Git、Python 3.12+、Rust 1.98.0、Visual Studio C++ 桌面工具和 Windows SDK。

```powershell
git clone https://github.com/WeiR-h/buwei.git
cd buwei
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1 -Release
```

`dependencies.lock.json` 固定官方 OctoSense、Rinx、Makepad、OctoScript 及相关框架提交。依赖保存在被忽略的 `.deps`；获取脚本核对提交和产品补丁摘要，并逐文件验证补丁结果。Rust 工具链和 Cargo.lock 随源码固定。

构建后使用 `tools/Start-BuWei.ps1 -Role organizer -Executable native/target/release/buwei-rinx-dual-host.exe`。参与者将角色改为 `participant`。首次运行不含登录、模型密钥或业务授权。

正式构建使用 `full-host`。本地测试接口仅在显式 `acceptance` 功能和启动参数同时开启时使用，公开运行包不包含该接口。源码检查、干净 Windows 构建、独立运行环境启动及包摘要核验由 Windows 工作流完成。

参见 [使用指南](FIRST-RUN.md) 和 [许可声明](../NOTICE.md)。

独立模型验收的 100 个虚构案例保存在 `native/tests/fixtures/independent-model-cases.json`。普通测试只检查案例格式；真实评测须由开发者显式开启模型评测模式，使用本人配置的模型服务。模型原始回答及评测记录保存在本机，应用操作仍须经过宿主授权。

意图层另有组织者、成员各 50 项独立案例：`native/tests/fixtures/independent-intent-holdout.json`。核心测试可输入固定事实与时钟，复现目标、提醒去重和任务恢复。`--intent-validation` 可将已有模型记录送入目标编辑器的同一字段校验，分别记录原模型结果和应用校验结果，不会调用模型或发送业务事件。
