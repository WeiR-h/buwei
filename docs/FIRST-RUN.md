# 首次运行

本预览通过 `--gui` 进入官方宿主，构建状态以验收记录为准。

```powershell
$profileDir = Join-Path $env:LOCALAPPDATA 'BuWei/preview-organizer'
New-Item -ItemType Directory -Force $profileDir | Out-Null
$env:BUWEI_PROFILE='organizer'
$env:BUWEI_RINX_MODE='official'
./native/target/debug/buwei-rinx-dual-host.exe $profileDir --gui
```

参与者使用另一资料目录，`BUWEI_PROFILE=participant`。
Rinx 登录服务器为 `matrix.rinx.chat`，浏览器认证由本人完成。
打开补位，查看账号及授权范围，再确认授权；登录不等于业务授权。

1. 组织者创建活动，同步活动，将第二个账号邀请进房间。
2. 参与者加入指定房间，预览本人报名时段和人数，再确认。
3. 组织者同步后按规则预览邀请，再确认发送。
4. 参与者读取邀请，预览接受或拒绝，再确认自己的回复。
5. 组织者核验服务端回复并更新席位。参与者取消也需预览确认。

待核实时使用“核实全部待恢复回执”，沿用原编号。重启后重新授权，
先核实旧操作。不要为绕过待核实记录删除数据库。v0.0.13 暂需手动同步。

模型可选，目前读取 Windows DPAPI 加密的 MiniMax 配置。独立配置入口
待补齐，没有密钥时先手动使用。不要上传密钥、会话或数据库到 Issues。

升级时关闭旧进程，使用 `tools/migrate.py` 复制并检查数据库。保留原目录
和旧入口，有效授权不迁移。迁移失败标记阻止新版写入。
房间创建或成员加入结果不明时先在 Rinx 核对；设置流程的自动恢复待验收。
