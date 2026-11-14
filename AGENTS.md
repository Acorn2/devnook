# DevNook

macOS 优先的本地开发工作台。产品名使用 DevNook，原始设计资料 `docs/DevPilot-PRD-Technical-Design-V1.0.md` 保留原名作为需求来源。

- 当前已实施 Phase 0–3；IDEA 插件属于后续阶段。
- `apps/desktop`：React + TypeScript + Tauri 2；`crates/devnook-core`：扫描、项目库、进程、IDE、安全策略、工作区编排与授权；`apps/cli`：clap CLI 和 stdio MCP 适配。业务逻辑必须在 Core，前端只展示和调用。
- 扫描只读、不跟随符号链接、不读取 `.env`、不执行任何项目命令。导入必须经过候选预览。
- 进程事实来自 macOS；不可用时显示错误，禁止用样例数据替代。未知归属只读。外部停止仅发送 SIGTERM，核验身份并要求确认；不得终止外部进程组。托管进程组须核验 runner 的微秒启动身份；SIGKILL 必须独立二次确认。
- 桌面是 UDS Core 客户端；Core 和服务 runner 均复用本 crate，不依赖窗口存活。不要安装 LaunchAgent。日志在 runner 中限量、标记流并脱敏后落盘。Keychain 保存敏感环境变量，SQLite 仅存引用。
- 配置保存不会启动命令。每次启动/重启需要绑定配置版本的短期确认；端口冲突必须阻断。不要将外部同端口进程归为托管。
- `pnpm check` 检查 TypeScript，`pnpm build` 构建前端；`cargo test -p devnook-core`、`cargo check --workspace` 检查 Rust。`pnpm tauri build --debug --no-bundle` 构建本地应用。测试临时数据不得写入真实应用数据库。
- 修改后更新 `docs/DEVELOPMENT-PLAN.md` 的实际验证结果。不改动原始需求文档，不默认安装 LaunchAgent，不提交或推送。
- 外观遵循 `PRODUCT.md` / `DESIGN.md`；各页面与浮层使用 `theme.css` 语义变量，支持浅色 / 暗色，不重新硬编码旧灰绿色。主题仅是本机展示偏好，不写入 Core 业务设置或服务配置。
- 界面语言属于本机展示偏好：首次只按 macOS 首选语言判断，`zh` 及其变体使用中文，其余使用英文；显式 `devnook.language` 选择优先。设置中即时切换，不重建页面、不触发业务保存。应用文案通过 `i18n.ts` 与集中词典翻译，项目名、分类值、路径、命令和原始日志保持原文；不把语言写入 Core、SQLite、服务配置或 MCP。
- 新包使用固定输出路径；构建且验证通过后清理本项目旧打包产物与旧验收 `.app` 副本，只保留最新可用包，避免累积占用空间。清理前核对字面量路径和运行状态；仍在运行的旧包待正常退出后再清理，不自动终止进程。
- macOS 发行区分 aarch64 / x86_64，Intel 使用 `target/x86_64-apple-darwin/release/bundle/` 和 Tauri 的 `_x64.dmg` 文件名。`site/release.json` 按架构维护 builds，登记一个候选不能重置其他架构的公开状态；只追加新架构资产，不覆盖已发布 ARM 包及其 SHA256SUMS。Intel 校验文件为 `SHA256SUMS-x86_64`。Release GUI 忽略 DEVNOOK_DATA_DIR，该变量仅 Debug 生效；正式包启动检查保持只读，临时业务数据须由隔离 Core / CLI 测试承担。
- `site/` 是独立的中英文 GitHub Pages 官网，`pnpm site:build` 输出 `dist/site/`，`pnpm site:check` 验证发布状态与静态链接。只使用演示截图，不连接真实 Core 或执行服务命令；`site/release.json` 必须反映实际签名、体积、校验和与公开状态。未上传的 DMG 不显示下载链接；发布步骤见 `docs/GITHUB-PAGES.md` / `docs/RELEASING.md`。

- 工作区自动展开服务依赖并按拓扑顺序等待 READY；回滚/停止只操作带本次 workspace_run_id 的实例，保留既有或后来重启的运行。Core 启动先捕获未完成编排，再恢复回滚。禁止自动强杀回滚失败的服务。
- MCP 默认只读，控制凭证仅从环境变量读取。Core 校验范围、依赖配置版本、撤销状态和确认令牌归属；不暴露任意 Shell、配置写入、外部进程终止或强停。审计不得记录凭证、环境值或完整请求参数。
- SQLite v3 升级 v2 前备份。Core UDS 协议 v10，不自动退出不兼容的旧 Core；同用户私有 UDS 为本机信任边界，不抵御同用户恶意程序伪装本地 GUI/CLI。
- 原生 GUI 的 setup 仅创建经过目录校验的 deferred Core 客户端，不在窗口创建前握手；协议 / 连接错误由界面展示并支持手动重连。CLI 保留启动时握手。不得以退出旧 Core、停服务或绕过协议检查来修复窗口启动。
- 扫描 depth=0 按项目特征和模块声明发现，识别后仅继续声明路径、常见组件容器和两层浅探，普通源码 / 素材 / 文档 / 隐藏目录默认剪枝；1–12 为深度补扫。已导入组件定点复查，不能因策略优化丢失已有证据。模块 glob 必须共享预算，逐段拒绝 symlink / .env / 依赖 / 所选根外路径，不求值任何项目配置。仓库/工作区内组件归到一个项目，被发现的独立嵌套仓库单列；未声明且藏在剪枝子树内的新项目需补扫。重复导入保留既有 ID 与偏好，不自动合并或删除项目，也不创建服务。
- 扫描和启动建议共用 Core 的本机服务资格规则；默认与补扫均排除明确的纯移动 / 桌面 GUI / 游戏客户端及其无服务聚合根。保留独立后端、Swift 服务端、Expo / React Native 开发服务器及明确 Web 服务脚本；不按语言或目录名推断服务，也不因新规则删除既有档案、配置或停止运行。规则范围见 `docs/service-eligibility-plan.md`。
- Phase 3 完整测试入口 `pnpm test`；CLI 构建 `pnpm build:cli`。MCP 接入说明见 `docs/PHASE-3.md`，验收不得修改用户真实 MCP 客户端配置或创建真实范围授权。

- 项目服务启动建议由 Core 只读识别已导入路径的当前清单，既有配置不被覆盖；建议与单行命令解析均不执行命令，不隐式切换 Shell。首次启动仍先保存配置再取得版本确认，高级参数默认收起。

- 服务“打开浏览器”仅供本地 GUI，由 Core 客户端组合现有 v10 只读接口；URL 只来自当前核验的服务监听，打开前复核 URL / PID / 指纹。只用固定 open 参数打开 HTTP / HTTPS 根地址，不接受任意 URL、预期端口或日志链接，不新增 MCP 能力，不自动启动或停止服务。

- 开发应用由 Core 只读发现系统登记与有限应用目录，校验 `.app` 元数据及可执行文件，不加载或运行应用代码；仅显式打开才调用固定 `/usr/bin/open` 参数数组。项目默认优先，新导入（含 Java）继承全局设置，不改写已有偏好；卸载保留偏好。应用发现/选择/设置接口仅供本地 GUI/CLI，MCP 保留既有固定 target 与 DEFAULT，不接受任意应用路径。

- 手动选择应用须通过 remember_development_app 在 Core 保存独立路径 / Bundle ID 记录，再读取列表；inspect_development_app 保持只读。记录位于现有 app_settings JSON 的私有字段，保存普通设置必须保留；不依赖项目默认，不把暂不可用当作删除，不改变 SQLite schema。

- 桌面“启动方案”位于服务管理内，沿用 workspace 数据与协议，不新增启动机制；侧栏不放监听筛选。项目列表端口状态与其他筛选组合，仅依据成功的 TCP 快照，未知 / 读取失败不得归为“未检测到监听”或“已停止”。

- 服务实际端口仅使用 Core 核验的 DEVNOOK + service_id 绑定；预期端口、cwd 匹配和 READY 不可替代。外部监听按 Core 的 project_id 汇总，来源工具不可靠时统一标为外部；显式手动确认只保存本次指纹关联，不接管、不创建服务。导入后失效旧快照并立即补刷，不能因已有刷新而丢弃请求。

- 项目服务页与项目监听状态仅统计 Core 的 process_kind=SERVICE；开发工具自身与未知用途监听只在完整端口管理中显示。逐 PID 按可执行文件及实际入口识别，父进程是 Codex / IDE 不能排除子服务；cwd 或手动确认项目归属均不等于服务证据。分类不影响 source、托管身份、can_stop 或控制权限。

- 外部服务可显式正常停止，或先保存独立配置再确认切换。切换由 Core 绑定原指纹、项目、监听、新服务及依赖版本 / 运行实例；先预检，再仅向旧 PID 发送 SIGTERM，等待其退出（含已终止但尚未回收的僵尸状态）及原 / 新预期端口释放，才复用托管启动。超时、身份变化或占用阻断，不停止外部进程组、不自动强杀、不重放观察命令或复制旧环境。确认只存内存、重启失效；MCP 不开放外部停止 / 配置建议 / 切换接口。
