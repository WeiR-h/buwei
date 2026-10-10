# 权限与隐私

本文说明补位原生 Windows 宿主扩展的数据流、凭据保存、Agent 来源和本机网络接口。运行包内附有本文件；固定版本的文件与摘要以该 Release 的 `release.json`、`SHA256SUMS.txt` 为准。

## 1. 账号与业务授权

Rinx 管理登录会话。补位从 Rinx 当前客户端取得账号，并向 `https://matrix.rinx.chat` 核验账号及设备，应用参数不能替代真实身份。

补位单独预览业务权限，授权最长一小时。撤销、账号切换、退出或到期后停止补位受授权约束的同步、模型请求和新动作；重启需重新授权。Rinx 自身的聊天登录与同步由其设置管理。已交给服务器的请求仍可能完成，补位保留编号供后续核实。

新邀请、本人报名和回复、活动卡片分享及文章发布绑定账号、活动、内容和操作编号。自动补位另行确认队列规则、发送时段、邀请期限和次数上限，规则变化后需重新授权。参与者亲自决定是否接受。执行遵循 `prepare → confirm → execute → reconcile → receipt`。

## 2. 数据保存和外部去向

| 数据 | 保存或发送位置 | 可见范围 |
|---|---|---|
| 活动、报名、邀请、回复及文章 | 本机业务数据库；确认执行后发送到官方 Matrix 服务 | 业务事件和活动快照为服务端可读格式，相关房间成员可读取；分享对象可见卡片上的时间、地点与名额 |
| 个人目标、长期偏好、反馈、建议与任务进展 | 本机按账号隔离的 SQLite 数据库 | 不写入活动房间快照；账号文件名的哈希用于隔离，不是数据加密 |
| 草稿、操作编号、回执、同步进度 | 本机资料目录 | 用于恢复与去重；发送结果不明时先核实原编号 |
| AI 输入与输出 | 本机功能记录；请求发送到 `https://api.minimax.cn/v1/chat/completions` | MiniMax 接收请求正文及用于鉴权的 API key；不发送 Matrix 登录凭据 |
| 启动日志、模型用量与共享预算 | 本机 `logs`、`data/model`；默认共享预算为 `%LOCALAPPDATA%\BuWei\model-budget\budget.db` | 日志可能含账号、路径、状态等诊断信息；提交问题前需脱敏 |

默认资料目录为 `%LOCALAPPDATA%\BuWei\v<版本>-organizer` 或 `v<版本>-participant`，也可通过启动参数指定。业务数据库位于 `data/v<版本>/native/rinx/<账号哈希>`，Rinx 资料位于 `data/v<版本>/rinx`，OctoSense 资料位于 `data/v<版本>/shell`。资料与程序分开保存，业务数据库没有整体加密。

本人请求“一句话建活动”、目标理解或自然语言报名时，模型接收填写的需求与补充回答。文本按原意发送，请不要填写密码、密钥或其他敏感内容。候补解释、活动问答和小记使用去身份事实，排除账号、姓名、房间编号和活动标题。活动问答还会发送本人填写的问题，问题原文不自动去身份，请勿包含敏感资料。小记不推断未经记录的到场或现场体验。

“主动 AI 分析”另行预览并确认，只发送去身份的类型、时段、人数统计及已确认目标字段。相关事实或目标变化后最多每分钟一次，相同依据复用结果；普通十秒同步不直接调用模型。模型只生成草稿、理解和解释，不能自行报名、选人、占位或发送；返回内容通过结构及业务规则检查。

个人推荐只使用本人已接入并核验的活动。取消不会自动改变长期偏好；更新长期偏好需本人明确选择，支持查看和撤回。其他 OctoSense 应用及系统助手的授权与模型配置应在各自设置中查看。

## 3. 密钥与登录会话

- **MiniMax key：** 配置窗口在本机使用 Windows DPAPI `CurrentUser` 加密，写入资料目录 `.secrets/minimax-cn.dpapi`。配置时不联网；请求模型时由宿主解密到内存，以 `Authorization: Bearer` 通过 HTTPS 发给固定的 MiniMax 地址。传输代码不跟随重定向。DPAPI 保护静态文件，同一 Windows 用户下有权限的程序仍可能解密，不能替代账号与电脑访问保护。
- **Rinx 会话：** 固定上游把 access token、refresh token（如有）及数据库 passphrase 序列化为 JSON，保存在 `data/v<版本>/rinx/<处理后的账号>/persistent_state/session`。该会话文件没有 DPAPI 加密；Windows 依靠资料目录继承的文件权限。Rinx 使用 passphrase 配置 SDK 存储，但 passphrase 同时保存在会话文件中，不能把整个目录视为加密备份。
- **本机连接凭据：** Octos 外部连接描述文件含 token，启用时位于 `%LOCALAPPDATA%\OctoSense\client-connection.json`。它依靠目录文件权限保护，不使用 DPAPI。宿主连接 token 保留在进程内。

业务授权保留在宿主，密钥与会话不作为应用或模型输入。多个补位入口共享本机模型预算，实际扣费以 MiniMax 账单为准。

## 4. Agent、运行依赖与下载摘要

补位逻辑编入 `buwei-rinx-dual-host.exe`；本产品流程不在首次使用时另行下载补位 Agent 可执行文件。Octos 内核随 Windows 包提供。源码构建通过 [bootstrap 工具](../tools/bootstrap.py) 获取固定提交，并校验框架版本及补丁摘要；内核由 [Build-Kernel](../tools/Build-Kernel.ps1) 从固定源码构建。

| 组件 | 官方源码 | 固定提交 |
|---|---|---|
| OctoSense 宿主 | [OctoSense](https://github.com/OctoSense-org/OctoSense) | `dba1933051cf3974d79bc1788071091b213dcd40` |
| Rinx 身份与消息 | [Rinx](https://github.com/hagency-org/Rinx) | `4b89097d8791a7190d01de1c576979c93df0013d` |
| Octos Agent 内核 | [octos](https://github.com/octos-org/octos) | `b0759a57719fd35b3a2da1c5d969bc67538ed516` |
| App Hub | [OctoSense-App-Hub](https://github.com/OctoSense-org/OctoSense-App-Hub) | `2e5768964f15916cdb66cc6824f7671dafc6445a` |

以下为 [v0.2.2 已发布预览包](https://github.com/WeiR-h/buwei/releases/tag/v0.2.2) 中已核对的 SHA-256；后续版本采用各自 Release 的清单，不复用旧版二进制摘要。

| 文件 | SHA-256 |
|---|---|
| `native/buwei-rinx-dual-host.exe` | `15e76477cd8a49ffb907937a4e2124ea982732f1e6b107e81431aa32edcf8a4c` |
| `native/octos-kernel.exe` | `518605ddf8e2aac198823db066c52e79877a718b3fade2210e8dc77ca2074151` |
| `native/octos-kernel.json` | `828cbad0b0548ef1c480a3b5e6a3c8dad8b854c0d0dc51241d526cce4d22ab8c` |
| `dependencies.lock.json` | `cc8263a40bb82c1fce0e954db00f0a7f5eab25f4dd651f8d8ad10378ab42f330` |

宿主启动随包内核前核验 `octos-kernel.json` 中的固定 revision 和 SHA-256；相同路径、大小、修改时间的已核验文件在进程内复用结果。摘要用于发现文件不一致，不是发行者签名。开发者显式设置 `OCTOS_APP_CORE_BIN` 会改用自选内核并绕过随包校验；`OCTOSENSE_KERNEL_ANY_REVISION=1` 放宽 revision 检查但保留摘要检查。日常使用沿用运行包默认配置。

宿主 App Hub 的默认目录源为 `https://raw.githubusercontent.com/OctoSense-org/OctoSense-App-Hub/main/`，目录、图片和选定包从该源读取。上游核验目录签名、应用准入记录及解包后的摘要；每个包摘要来自已验证目录，随版本变化。`OCTOSENSE_HUB`、宿主的 `hub.txt` 可指定镜像，`OCTOSENSE_HUB_ANCHOR` 可替换信任根；这些是独立的高级配置。下载其他应用及其 Agent 不会把本原生补位项目变为 App Hub 商店应用。

## 5. 本机监听与代理

| 接口 | 默认状态与地址 | 访问控制及用途 |
|---|---|---|
| 补位界面与业务线程 | 同一宿主内的命令/状态通道 | 无单独的补位 HTTP 业务接口；实际操作仍核验账号与授权 |
| OctoSense 窗口客户端 hub | `127.0.0.1:8765–8784` 中可用端口 | 进程客户端以一次性 `X-Studio-Token` 绑定；带浏览器 Origin 的握手拒绝，进程内模块没有可被 socket 绑定的 token |
| Octos 内核 RPC | 默认私有子进程 stdin/stdout | “Talk to Octos”默认关闭；启用后为 `127.0.0.1` 的 HTTP/WebSocket，分开宿主 token 与外部 token，外部连接只开放 UI Protocol；可关闭或轮换 token |
| App Hub 应用资源 | 打开相应应用时为 `127.0.0.1` 随机端口 | 提供该应用包内资源，无账号鉴权，阻止读取包目录之外的路径；应用关闭时停止服务 |
| Makepad `--remote` 诊断 | 默认关闭；测试时为 `127.0.0.1` 随机或指定端口 | 可读取画面、日志并输入鼠标键盘，没有补位业务授权认证。显式 `HOST:PORT` 可监听其他接口；不要对外开放。正式启动脚本清除 `MAKEPAD_REMOTE` 且不传 `--remote` |
| 故障测试 CONNECT 代理 | 仅测试工具启动，`127.0.0.1` 随机端口 | 无客户端鉴权，仅允许 `matrix.rinx.chat:443` 与 `auth.matrix.rinx.chat:443`；不解密或记录 TLS 正文，测试结束关闭；正式包不启动此代理 |

正式运行包不启用 `acceptance` 特性中的本地验收控制。启用托盘值守后，隐藏窗口继续运行已授权业务；托盘“暂停值守”撤销补位授权，“退出”停止程序。

网络代理是出站路由设置。Octos 支持 `MAKEPAD_OCTOS_PROXY`，Rinx 等 HTTP 客户端还可能使用其库支持的系统或环境代理。补位不安装系统代理、不监听公网代理端口；HTTPS 目标仍执行 TLS 校验。使用会解密 HTTPS 的代理时，代理运营方可能看到请求及凭据，应由用户自行选择可信配置。

## 6. 查看、删除与备份

1. **停止主动操作：** 在补位撤销授权或托盘暂停值守；真正退出程序后再整理资料。关闭窗口并隐藏到托盘不等于退出。
2. **删除偏好：** 在“我的目标”修改、暂停或删除目标；使用偏好恢复默认、反馈撤回入口。物理删除所有本机资料需在退出后删除对应资料目录；SQLite 普通删除不保证旧页面已安全擦除。
3. **移除模型 key：** 退出后删除该资料目录的 `.secrets/minimax-cn.dpapi`；如需使泄露的 key 失效，在 MiniMax 控制台撤销或更换 key。删除本机文件不会撤销服务端 key。
4. **移除登录：** 先在 Rinx 退出登录，必要时在官方账号设备管理中撤销对应会话。随后可删除该资料目录的 Rinx 会话与缓存；单删文件不能保证服务端 token 立即失效。
5. **处理备份：** 升级前关闭旧版并备份，业务授权不迁移。源码或公开资料备份应排除 `.secrets`、Rinx 会话、数据库、聊天缓存、原始日志与开发输出。需要恢复业务时，把完整私密备份保存在自己控制且有访问保护的位置。删除本机副本不会删除云端备份或已经发送的房间事件。

发送结果不明时保留原操作编号与数据库，使用“待核实”恢复入口；活动创建不明时使用“恢复待核实的活动创建”。核实结束后再删除资料，避免丢失恢复依据后重新发送。

发布工具按白名单打包并检查私密内容；本文件随运行包的 `docs/PRIVACY.md` 交付。向 [Issues](https://github.com/WeiR-h/buwei/issues) 反馈时提供脱敏后的问题步骤，不附密钥、会话或原始数据库。
