# Phase 2：Service Supervisor

当前授权：按现有需求完成服务启动、停止、重启、日志、端口冲突、就绪检测、测试与开发记录。不提交、推送、部署，不实现 CLI/MCP/IDE 插件/工作区依赖。

## 实施切片

1. 服务领域模型、SQLite v2 增量迁移与 Phase 1 备份，独立历史运行记录。
2. 显式配置 executable + args / Shell Mode、cwd、非敏感环境变量、Keychain 引用；启动前预览与配置版本确认。
3. 同一应用可执行文件内部 Core/runner 模式：私有 UDS、单实例锁、当前用户校验；GUI 关闭后继续运行，Core 重启后重连 runner。
4. runner 创建独立进程组，管理实际子进程和后代；实时脱敏、限量 stdout/stderr；PROCESS / PORT / HTTP / LOG_PHRASE 就绪检查。
5. 生命周期与错误路径：端口冲突、启动失败、就绪超时、崩溃、外部退出、父进程退出留子进程；TERM 宽限与独立强制确认。
6. 中文服务页面、配置编辑、运行详情、历史日志、状态与端口来源联动。
7. 隔离集成测试、前端构建、桌面打包、原生关键路径与开发记录。

## 数据与回退

迁移只增加服务配置/运行表，原项目档案不覆盖。有 Phase 1 数据时先生成一次 SQLite 备份。服务配置移除为归档，保留历史和日志；有活动服务时阻止移除项目。回退程序前先正常停止托管服务，保留 v2 库，使用独立目录打开 v1 备份。

## 安全与运行边界

- 只允许启动用户登记并确认的服务，扫描不执行任何命令。
- runner 是自有进程组的稳定锚点；PID/PGID/UID/微秒启动时间不匹配时拒绝信号。
- Core 与 runner 通过同一数据库交换可恢复事实；展示状态时核对实际 runner 存活，不把上次 READY 当作事实。
- 日志仅写脱敏文本，超过单行上限丢弃整行；每个运行限制日志大小并保留历史。HTTP 仅访问 loopback，不跟随重定向。
- Keychain 失败时明确报错，不降级成明文。测试注入内存 SecretStore，不读写用户钥匙串。
- 不安装系统服务或自启动配置。Core 按需启动，所有服务停完且无客户端请求一段时间后可退出。

## 验收

2026-11-12，macOS Apple Silicon。

- 已完成全部 7 个实施切片。Phase 1 的 8 项回归、Phase 2 的 10 项集成、真实 JVM 服务测试 1 项，共 19 项通过。5 个 ignored 函数为测试专用子进程入口，由集成测试实际调用。
- Node HTTP 服务、Python HTTP 服务和编译后的 Java ServerSocket 服务均实测；服务停止后端口消失。Maven/Gradle 使用相同 executable + args 机制，但未启动具体 Maven/Gradle 项目。
- 覆盖 PROCESS、PORT、HTTP、LOG_PHRASE；初始就绪、健康丢失降级/恢复、就绪超时停止、进程立即异常退出、缺失可执行文件、占用端口阻断。
- 覆盖正常停止清理后代、父启动进程退出后保留子进程、忽略 TERM 阻止重启、宽限后独立确认强制停止、确认令牌单次有效、配置版本变化拒绝旧确认。
- 覆盖当前 runner 身份不符时 UNKNOWN 且拒绝信号、移除活动项目/配置被阻止、敏感值只存引用、保留旧 Keychain 值的配置版本复制、日志脱敏/限量/查询/单次清理、v1 备份和增量迁移。
- UDS 测试验证同一 Core 被多个客户端共享；客户端退出不影响服务；杀死测试 Core 后 runner 和日志继续，Core 重启后恢复同一实例；大于 8 KiB 的配置/响应完整传输。
- 原生 GUI 在独立数据目录 .tmp/phase2-native-qa 验收：扫描导入本仓库 → 添加 Node 服务 → 保存未执行 → 核对启动命令/目录/环境变量 → 确认启动 → 从 RUNNING 变为 READY → stdout/stderr/心跳日志 → 确认重启生成新 PID 与独立历史。
- GUI 退出由 localhost 状态确认 exit code 0；退出后实际 HTTP 仍返回 200 且日志持续增长。最终通过同一 Core 正常停止该验收服务，状态 STOPPED、退出码 -15，保留 2 次运行历史，并退出测试 Core。
- TypeScript、Vite production build、Rust workspace、Prettier、rustfmt 和 debug macOS .app 打包通过。产物约 37.8 MiB。

修复的集成问题：正常 TERM 在子进程先退出时被误判为 CRASHED；macOS accepted UDS 继承非阻塞导致大响应只发送首个 8 KiB；已分别修正并加入回归验证。进程组成员使用 libproc，避免在被终止组内启动 ps。就绪超时独立于探测间隔，停止宽限按毫秒核对。

限制：未实测 Intel、macOS 12、真实 Keychain 权限交互和具体 Maven/Gradle 项目；测试 Keychain 使用内存替身。仅支持 loopback HTTP，不支持 TLS/认证健康端点。会主动脱离进程组的命令需使用前台模式；Gradle 建议显式 --no-daemon。没有签名、公证、发布、LaunchAgent、CLI/MCP/工作区/IDE 插件。
