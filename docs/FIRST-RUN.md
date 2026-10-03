# 首次运行

v0.1.0 通过 `--gui` 进入官方宿主；运行包已在独立全新 Windows 环境启动。

```powershell
$profileDir = Join-Path $env:LOCALAPPDATA 'BuWei/v0.1.0-organizer'
New-Item -ItemType Directory -Force $profileDir | Out-Null
$env:BUWEI_PROFILE='organizer'
./native/target/debug/buwei-rinx-dual-host.exe $profileDir --gui --official-rinx
```

参与者使用 `BuWei/v0.1.0-participant` 资料目录，`BUWEI_PROFILE=participant`。
随包入口按版本自动分开资料目录；升级不会自动覆盖旧版资料。
Rinx 登录服务器为 `matrix.rinx.chat`，浏览器认证由本人完成。
首次看到 “Let Rinx's agent start?” 弹窗时选择 **Don't allow**。
该弹窗控制后台 Agent；本版提供本人登录、报名和消息操作，后台唤醒
安排在后续版本。关闭弹窗后在底部 Rinx 窗口继续登录。
打开补位，查看账号及授权范围，再确认授权；登录不等于业务授权。

1. 组织者创建活动，同步活动，将第二个账号邀请进房间。
2. 参与者加入指定房间，预览本人报名时段和人数，再确认。
3. 组织者同步后按规则预览邀请，再确认发送。
4. 参与者读取邀请，预览接受或拒绝，再确认自己的回复。
5. 组织者核验服务端回复并更新席位。参与者取消也需预览确认。

待核实时使用“核实全部待恢复回执”，沿用原编号。重启后重新授权，
先核实旧操作。不要为绕过待核实记录删除数据库。授权期间每 10 秒自动同步；缺失历史或时间不可核实时保留原记录。

模型可选，目前读取 Windows DPAPI 加密的 MiniMax 配置。在“帮助与模型”中点击“填写或更新本机 MiniMax 密钥”，
或使用随包配置工具。没有密钥时先手动使用。不要上传密钥、会话或数据库到 Issues。

升级时关闭旧进程，使用 `tools/migrate.py` 复制并检查数据库。保留原目录
和旧入口，有效授权不迁移。迁移失败标记阻止新版写入。
房间创建或成员加入结果不明时先在 Rinx 核对；设置流程的自动恢复待验收。

## Windows 运行包

解压完整目录后双击“启动组织者.cmd”或“启动参与者.cmd”。首次启动
不含登录、授权或模型密钥；两个入口使用独立的资料目录。不能单独移动
可执行文件，字体、图片与配置工具必须保留。模型配置工具只在本机弹窗
填写密钥，并由当前 Windows 用户加密保存。

升级迁移：关闭旧版后运行 `python tools/migrate.py 旧资料目录 新资料目录
--from-version 0.0.16 --to-version 0.1.0`。核实迁移记录与数据库后用新
资料目录启动，重新查看授权范围并沿原编号核实。保留旧目录和旧入口。
