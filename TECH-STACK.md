# DevNook 技术栈

官网采用原生 HTML / CSS / JavaScript 与 Node 标准库静态生成，不新增 npm 包、前端框架或运行时 API。`site/` 同源提供中英文 HTML、语义主题和自有图片；支持 GitHub Pages 项目子路径、移动导航、减少动态效果和存储 / 剪贴板失败降级。发布清单由本地 Release 元数据生成，并在部署前校验；GitHub Actions 的 Pages 官方 Action 固定 commit。Release 使用 thin LTO、`codegen-units = 1` 与 `strip = "symbols"`，沿用默认速度优化和 panic unwind。

- 中英界面采用集中 TypeScript 词典、React 语言订阅及 localStorage，无新增 npm 依赖。macOS 首选语言使用已有锁定版本 `objc2-foundation` 0.3.2 的 `NSLocale.preferredLanguages`，只在桌面适配层读取，不调用 Shell 或改动 Core。

- Tauri 2：macOS WebView 桌面壳与类型化命令入口。
- React 19 + TypeScript + Vite：前端交互与构建。
- 项目展示偏好使用 React hook 与版本化 localStorage；分类编辑复用共享 Select 和现有 Core 项目字段，无新增依赖、数据库表或协议。
- 原生 CSS 变量：中英双语工作台、响应式布局，避免为第一阶段增加组件框架。
- 浅色 / 暗色共用语义 CSS 变量；OKLCH 配色带旧 WebView 十六进制兼容值。localStorage 保存独立外观偏好，原生 Tauri setTheme 同步应用标题栏，无新增依赖。
- Rust 2021 workspace：独立 Core crate 与薄 Tauri 适配层。
- rusqlite（bundled SQLite）：本地项目、设置和审计持久化，不依赖外部数据库。
- serde / serde_json：IPC 与项目清单；uuid：档案与短期确认凭证；libc：本机 UID、信号。
- tempfile：测试隔离目录和数据库。
- Prettier / rustfmt：统一新工程的格式，格式检查不跳过类型或构建验证。
- 外部服务切换复用 Core 的服务操作锁、内存确认、单 PID 身份检查和 runner；使用既有 libproc / 固定 ps stat 核验退出，标准库有界等待端口释放。前端复用服务编辑器并提供独立 PID 确认，无新增依赖、数据表或网络接口；SQLite v3 / UDS v10 保持。
- Phase 2 以标准库 UnixStream / UnixListener、文件锁实现私有 IPC，Core 与 runner 使用同一可执行文件内部模式，不增加系统安装。
- security-framework 3：macOS Keychain，敏感值不进入 SQLite，配置版本隔离引用；测试使用内存 SecretStore。
- url 2：校验 loopback HTTP 目标；健康检查使用标准库限时 TCP，不增加 HTTP 客户端或 TLS 栈。
- macOS libproc：微秒启动身份与进程组成员；通过 SDK 的 PROC_PGRP_ONLY 查询，避免在被终止组内启动 ps。

开发依赖安装在本项目，不安装全局包。Rust 缺失时可在 `.tmp/toolchain` 安装隔离工具链验证；开发者已有稳定 Rust 时直接使用全局工具链。

扫描发现使用 Rust 标准库有界遍历、单次目录枚举缓存与现有 serde 项目 payload。Core 直接复用锁定的 toml 1.2 / quick-xml 0.42 / glob 0.3 解析 TOML、XML 和路径模式，不执行工程配置，不增加后台服务或数据库 schema。YAML、Gradle、Go、CMake、Meson 仅处理静态字面量子集。ScanProgress 增加默认兼容的 entries 计数；Core UDS 当前为 v9，保证新版不会默默连接旧扫描策略，旧 Core 不自动退出。SQLite 仍为 v3。

- Phase 3：clap 4 derive 负责 CLI 参数与帮助；sha2 0.10 负责高熵 MCP 凭证摘要，不使用摘要存储用户密码。
- MCP 使用标准库 stdio + serde_json 实现有界 JSON-RPC 适配，遵循官方 transports/lifecycle/tools 规范；只声明 tools 能力，无网络监听、资源订阅或后台通知协议。具体版本固定在 Cargo.lock。

项目服务简化继续使用现有依赖与数据表；Core 新增只读 service_drafts 和 parse_service_command，前端不实现命令识别或 Shell 求值。高级配置使用原生 details/summary 渐进展开。

本机服务资格复用上述 JSON / TOML / XML 解析器与静态字面量 tokenizer；扫描、分组和启动建议共用 Core purpose 判定，不安装项目工具、不读取普通源文件内容、不运行原生构建器。没有新增依赖或 schema；UDS v9 阻止旧扫描规则被新版客户端静默使用。

开发应用识别通过 macOS LaunchServices 与 CoreFoundation C 接口读取系统登记及 XML / binary plist，FFI 函数签名与对象所有权规则对照本机 SDK；不加载应用代码，无新增依赖。沿用 JSON 偏好存储和固定 open 参数数组，SQLite 仍为 v3；应用接口和路径目标仍由当前 UDS v9 门禁保护。

手动应用记录复用 app_settings JSON，无 SQLite schema 迁移；Core 合并保留私有缓存字段，客户端 Settings 无需携带此字段。remember_development_app 自 UDS v7 引入、在当前 v9 中仍仅本地可用，inspect_development_app 仍为只读。
