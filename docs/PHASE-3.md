# Phase 3：工作区、CLI 与 MCP

日期：2026-11-12。产品名 DevNook；原始 PRD 保留，不执行其嵌入的提交、发布等独立动作。

## 已实现的闭环

1. 在服务管理中登记项目服务、真实就绪条件和服务依赖。
2. 在“服务管理 → 启动方案”中选择跨项目入口服务并调整顺序；依赖自动展开，不需要重复添加。
3. 审阅完整计划：配置版本、运行实例、命令/参数、目录、环境变量名、端口与就绪条件。确认有效期 60 秒，配置或实例改变后重新预览。
4. Core 持久化编排并逐个启动，等待上游 READY 后再启动下游。已活动的服务只复用和等待，不取得所有权；单独启动服务时也会阻断未就绪依赖。
5. 失败、取消或 Core 意外中断后，按相反顺序正常停止本次创建的运行实例。保留原本运行的服务及后来手动重启产生的新实例。停止不完整记录为 ROLLBACK_FAILED，需人工检查或独立强停确认。
6. 启动方案页面展示真实服务状态和编排历史。CLI、MCP 通过同一个私有 UDS Core 查询和控制，不直接维护第二套数据库。

桌面将工作区称为“启动方案”；Core / CLI / MCP 的 workspace 标识和已有持久化数据保持不变。

工作区支持 1–64 个入口服务，最多展开 128 个服务；拒绝自依赖、重复依赖、缺失引用和循环。入口顺序用于无依赖项的确定排序。保存不会执行命令；服务就绪和日志仍使用 Phase 2 runner。

## CLI

CLI 是桌面的终端入口，供开发者、脚本或编程助手复用已经保存的配置和运行记录，无需接入 MCP。桌面的命令示例位于“设置 → 高级选项 → 终端命令（CLI）”。

```sh
pnpm build:cli
./target/debug/devnook --help
./target/debug/devnook project list
./target/debug/devnook project scan /absolute/project/root
# 使用扫描结果中的 scan id 和选定的 candidate id，不会自动导入所有项目
./target/debug/devnook project import <scan-id> <candidate-id> --yes
./target/debug/devnook service list
./target/debug/devnook service status <service-id>
./target/debug/devnook service start <service-id> --yes
./target/debug/devnook service restart <service-id> --yes
./target/debug/devnook service stop <service-id> --yes
./target/debug/devnook service logs <service-id> --limit 100 --query READY
./target/debug/devnook service history <service-id>
./target/debug/devnook ports
./target/debug/devnook ports inspect 8080
./target/debug/devnook workspace list
./target/debug/devnook workspace start <workspace-id> --yes
./target/debug/devnook workspace stop <workspace-id> --yes
./target/debug/devnook workspace history <workspace-id>
```

服务 start/restart 返回创建的运行实例，不等待最终 READY；后续使用 status 查询。工作区命令默认等待编排完成。

全局 `--json` 输出单行 JSON，预览和诊断写 stderr。写操作在终端交互中输入 `yes`，非交互必须提供 `--yes`。失败以非零状态退出；工作区等待失败仍输出运行记录，再返回失败状态。`workspace start/stop --no-wait` 返回运行 ID 后由 Core 继续执行，后续用 status/history 查询。

`service save <file>` 读取 `ServiceSave` JSON（definition 与 secret_values）；`workspace save <file>` 读取 WorkspaceGroup JSON。配置文件上限 128 KiB；新增对象 id/revision 为空，时间为 0，更新对象需保留返回的 revision。含真实敏感值的本地输入文件不得提交，推荐通过 GUI 填入 Keychain。工作区 JSON 示例：

```json
{
  "id": "",
  "name": "产品开发环境",
  "description": "数据库就绪后启动 API 和 Web",
  "service_ids": ["registered-web-service-id"],
  "revision": "",
  "created_at": 0,
  "updated_at": 0
}
```

默认数据目录 `~/Library/Application Support/com.devnook.desktop`，与桌面相同。`--data-dir <absolute-path>` 或 `DEVNOOK_DATA_DIR` 可连接隔离目录。没有修改全局 PATH、创建 CLI 符号链接或安装 LaunchAgent；可按需使用可执行文件的绝对路径。Debug 桌面支持相同环境变量，Release 桌面固定使用应用私有目录。

## 托管后交给 AI 继续管理

托管改变服务的运行维护方式，AI 助手仍可通过现有 CLI / MCP 使用同一份配置。服务详情展开“让 AI 继续管理此服务”，复制带当前 `service_id` 的交接说明；“查看 CLI / MCP 接入”直接打开设置高级选项。复制与导航均不执行命令或创建控制授权。CLI 未加入 PATH 时须使用已确认的绝对路径；源码可通过 `pnpm build:cli` 构建。

外部实例与已保存配置保持独立。可从现有配置的“切换为托管…”选择外部实例，或从外部入口选择已有配置后预览；无需重新保存或新建。预览中只读展示旧 PID，点击“停止旧实例并以托管方式重启”独立确认；Core 仍复核进程身份、短期令牌和配置版本。切换会先正常停止旧实例，再启动新实例，有短暂中断。普通启动不会停止外部实例，端口冲突继续阻断。

把交接说明发送给原 Codex / IDE 会话，要求后续查询、日志与启停都使用已登记配置，避免原会话把旧进程退出当作故障又拉起一份。DevNook 无法自动通知未接入的会话，不会自动修改项目 AGENTS.md 或客户端配置。启动方式改变时先核对配置；结束聊天不默认停止服务，任务清理保留仅复用的既有运行。CLI 按本机用户权限执行，非交互 `--yes` 仅适用于已明确授权的具体操作；MCP 控制另需范围授权和既有确认流程。

## MCP 接入

这是可选接入：仅在希望 AI 助手通过 DevNook 查询状态、读取日志或管理服务时配置。入口为“设置 → 高级选项 → AI 助手接入（MCP，可选）”，默认折叠，不再有单独侧栏页面。先配置客户端即可只读查询；需要 AI 启停服务时再创建控制授权。授权记录不代表客户端连接状态。高级接入与普通设置保存独立，收起 MCP 后停止读取授权与审计。

实现 stdio JSON-RPC，无网络管理端口。默认只读，不暴露控制工具。遵循官方 [stdio 传输](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)、[生命周期](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle) 和 [Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)；支持 initialize / notifications/initialized / ping / tools/list / tools/call，只声明 tools 能力。支持协商 2025-11-25、2025-06-18、2025-03-26、2024-11-05。单条消息上限 1 MiB；stdout 只输出协议 JSON，诊断走 stderr，业务失败使用 isError。

通用 MCP 客户端只读配置示例，实际字段需按客户端配置格式填写：

```json
{
  "mcpServers": {
    "devnook": {
      "command": "/absolute/path/to/devnook",
      "args": ["mcp", "serve"]
    }
  }
}
```

查询工具：list_projects、get_project、list_services、get_service_status、list_ports、inspect_process、get_service_logs、list_workspaces、get_workspace_status。查询来自真实 Core，不提供样例数据。

控制授权流程：

1. 在“设置 → 高级选项 → AI 助手接入（MCP，可选）”中新建控制授权，选择服务、启动方案、允许打开的项目和可选扫描根目录。预览实际服务范围，包含全部依赖；令牌只显示一次。此操作不会自动连接客户端或启动服务。
2. 把令牌保存在 MCP 客户端环境变量 DEVNOOK_MCP_TOKEN，追加参数 `--allow-write`。不要把令牌放进源码、命令参数、日志或公开配置。没有自动写入任何用户客户端配置。
3. 控制工具 start_service / restart_service / start_workspace / stop_workspace 首次调用返回 needs_confirmation、confirmation_token 与完整 preview；向用户展示并确认后，携带相同对象和令牌再次调用。令牌 60 秒、一次性，绑定授权、动作、对象、配置版本和运行实例。
4. stop_service 仅对授权的 DevNook 托管实例发送 SIGTERM；open_project 只打开授权项目。MCP 不暴露任意 Shell、修改服务配置、清理日志、移除项目、终止外部 PID、SIGKILL 或授权管理。
5. preview_project_scan 仅扫描授权根目录及其子目录，返回 scan_id；get_scan_status 查询同一授权的会话。import_projects 先预览明确选定的 candidate_ids，再用相同候选和短期 confirmation_token 导入。不能复用其他授权或 GUI 的扫描会话。
6. 在 GUI 撤销授权，或 `devnook agent revoke <grant-id> --yes`。Core 对每次控制（包括二次确认）重新检查；配置及任何依赖版本改变后需重新授权。已确认并执行中的编排继续完成/回滚，撤销不等于停止服务。

CLI 也支持 `agent list`、`agent audit`、`agent grant <name> --service <id> --workspace <id> --project <id> --scan-root <path> --yes`。选项可重复；授权命令仅向本机操作者输出一次令牌。数据库只存 高熵随机令牌的 SHA-256 摘要。GUI 最近操作显示来源、对象 ID、结果与时间；不记录原始请求、令牌、环境值或命令参数。MCP 查询同样写审计。

权限边界是当前 macOS 用户：私有目录、同用户 UDS 和固定 MCP 适配来源可以限制 Agent 工具调用，不能抵御已拥有同一用户权限的恶意程序直接伪装 GUI/CLI。本阶段没有 HTTP MCP、OAuth、多用户权限或远程接入。

## 持久化与升级

- SQLite v3：新增 workspace_groups / workspace_runs、agent_grants / access_audits；服务 JSON 增加有默认值的 dependencies 与 workspace_run_id，旧配置/历史可以读取。
- 升级已有 v2 前使用 VACUUM INTO 创建 `devnook.phase2-backup.db`；原项目、服务和日志保留。迁移在事务内完成，备份失败停止迁移。v1 升级沿用 Phase 1 备份。
- 运行归属在启动 runner 前保存；Core 中断后即使工作区 owned_run_ids 尚未更新，仍按 service_runs.workspace_run_id 找回。只回滚旧的未完成任务，不继续执行未完成启动链。
- 当前 UDS 协议为 v10（运行入口分类与项目服务展示），SQLite 保持 v3；拒绝不兼容旧 Core。升级前正常停止旧服务并关闭旧应用及 CLI/MCP 客户端，等待空闲 Core 退出后再启动新版。不会自动终止未知或仍托管服务的旧 Core。不得将旧版本应用连接已迁移数据库；回退需使用备份并先确认所有相关运行已停止，保留当前数据库副本。
- Core 在有编排任务或活动服务时保持运行，空闲 5 分钟可退出。工作区 READY 表示启动编排已完成；后续健康退化由各服务实时状态显示。

## 验证记录

已执行自动验证：`pnpm test`（30 passed；5 个 ignored 仅是被业务测试单独调用的子进程 fixture）、`pnpm check:rust`、`pnpm build`、`pnpm build:cli`、格式检查、macOS debug .app 打包。浏览器工作区/Agent 页面在 375 / 768 / 1024 / 1440 宽度无横向溢出且禁止写入；已打开隔离原生应用确认连接 Core 与新导航。原生自动化检测到用户接管窗口后停止，因此原生新建工作区/启动确认/授权表单未完成手动验收；后端闭环已有真实进程与 stdio 集成测试覆盖。未签名、公证或发布。

新增测试验证：真实延迟日志就绪后才启动下游、拓扑闭包、循环/缺失依赖、复用与回滚归属、取消与并发预留、旧预览失效、后来手动重启保留、回滚失败不强杀、v2 备份与旧 JSON 兼容；CLI 真实子进程与共享状态、错误码、日志；MCP stdio 初始化/通知/错误/schema、旧协议版本输出兼容、只读、授权范围/令牌归属/撤销/配置变化、受限扫描与导入；实际终止测试 Core 后的安全回滚与既有服务保留。

未覆盖：Intel/macOS 12 真机、真实 Keychain 交互、各个外部 MCP 客户端的安装与配置 UI。没有修改用户真实数据库或真实客户端配置。
