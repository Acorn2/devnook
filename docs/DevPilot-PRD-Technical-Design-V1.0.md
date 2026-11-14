# DevPilot 产品需求与技术开发设计文档 V1.0

> **项目代号：** DevPilot\
> **产品定位：** Local Development Workspace & Service Manager\
> **文档类型：** 产品需求 + 技术架构 + 工程实施方案\
> **版本：** V1.0\
> **日期：** 2026-11-12\
> **目标平台：** macOS 优先（Apple Silicon / Intel）\
> **目标读者：** 产品开发者、Codex、后续贡献者

------------------------------------------------------------------------

## 0. 给 Codex 的实施说明

本文件是 DevPilot V1.0
的产品和工程基线。开始编码前，必须先完整阅读本文件，并按"阶段一 → 阶段二
→ 阶段三"的顺序实施。

**不要一次性实现全部规划。** 第一轮只交付阶段一的可运行
MVP，必须先完成项目扫描、项目管理、IDE
快捷打开、端口扫描、进程详情和安全停止。阶段一验收通过后，再实现服务托管。MCP
和 IntelliJ IDEA
插件属于后续阶段，不允许为了提前展示功能而用假数据模拟成功。

实施约束：

1.  以 macOS 原生进程为管理对象，不使用 Docker / Docker Compose。
2.  不修改被扫描项目的源码、配置文件或目录结构。
3.  扫描项目时只读取文件，不执行
    `npm install`、`pnpm dev`、`mvn`、`gradle`、Python 脚本等命令。
4.  端口扫描结果、托管服务运行记录、IDEA
    运行实例必须区分来源，不能混为一谈。
5.  不允许自动终止无法确认归属的外部进程。
6.  所有服务状态必须来自实际进程/端口/健康检查，不得只依赖 SQLite
    中保存的上次状态。
7.  GUI、CLI、MCP 后续必须调用同一个 Rust
    Core，不允许各自实现独立的进程管理逻辑。
8.  第一版只保证 macOS；跨平台接口可以预留，但不要提前引入 Windows/Linux
    的复杂实现。
9.  不要复制三个参考仓库的整个项目。先分析实现和许可证，再提取必要的设计、算法或代码片段。
10. 每个阶段必须有测试和验收记录，不能只生成 UI 截图就宣称完成。

------------------------------------------------------------------------

# 1. 产品概述

## 1.1 产品定位

DevPilot 是一个本地开发环境控制中心，统一管理开发者电脑中的：

-   本地项目目录及项目档案；
-   Java、Node.js、Python 等项目；
-   IntelliJ IDEA、VS Code、Cursor 等开发工具；
-   项目启动任务和服务；
-   本机 TCP 监听端口；
-   由 DevPilot、IDE、Terminal、Codex 等不同来源启动的进程；
-   多项目工作区及依赖启动顺序；
-   CLI 和 MCP 自动化入口。

DevPilot 不是 IDE，不替代 IntelliJ IDEA、VS
Code、Cursor；不是容器平台，不要求项目迁移或 Docker
化；也不只是一个端口查看器。

**核心目标：让开发者从一个入口发现项目、打开
IDE、启动服务、查看端口、识别进程、停止和重启服务。**

## 1.2 核心用户问题

  -----------------------------------------------------------------------
  当前痛点                            DevPilot 对应能力
  ----------------------------------- -----------------------------------
  项目分散在多个目录，记不清路径      扫描目录、项目库、搜索和收藏

  启动 Node 项目需要先打开            项目服务配置，一键启动
  IDE，再切换终端输入命令             

  Java 服务依赖 IDEA Run              第一阶段支持命令行启动；后续通过
  Configuration                       IDEA 插件读取和调用 Run
                                      Configuration

  Codex 启动了服务但没有登记          扫描本机端口和进程，识别外部进程

  端口被占用，不知道属于哪个项目      端口 → PID → 进程树 → 工作目录 →
                                      项目关联

  服务启动后不知道是否真正可用        Process / Port / HTTP / Log Phrase
                                      就绪检查

  多个项目需要按顺序启动              Workspace Group 和任务依赖

  不清楚哪些服务可以停止              区分托管服务、IDE
                                      托管服务、外部发现服务

  希望 Codex 直接操作服务             后续提供 MCP 工具和 CLI
  -----------------------------------------------------------------------

## 1.3 产品原则

1.  **Local-first：** 项目路径、配置、运行记录默认只保存在本机。
2.  **不接管项目：** 不修改项目技术栈，不改造项目启动方式。
3.  **先发现，再归属：** 进程识别不确定时显示"待确认"，不擅自归属。
4.  **状态以事实为准：** 进程、端口、健康检查是运行状态的依据。
5.  **危险操作可控：**
    外部进程、强制终止、命令修改、批量操作必须有确认策略。
6.  **统一 Core：** GUI、CLI、MCP、IDE 插件通过统一接口协作。
7.  **渐进式复杂度：** 第一阶段先做好项目和进程发现，再增加托管、IDE
    集成和 Agent 自动化。

------------------------------------------------------------------------

# 2. 开源项目参考与借鉴边界

本项目不是把三个仓库直接合并，而是分别借鉴其成熟的产品能力和工程设计。

## 2.1 CodeDeck

仓库：https://github.com/JadnK/CodeDeck

重点借鉴：

-   扫描一个基础目录，识别多个候选项目；
-   扫描结果预览后再选择导入；
-   项目名称、路径、语言、框架和工具标签；
-   项目偏好的 IDE；
-   保存项目命令、工作目录和环境变量；
-   项目详情、实时进程输出和停止操作；
-   Launch Sets：组合多个项目、命令、IDE、终端和 URL；
-   Tauri 2 + Rust + React + TypeScript 的桌面架构。

CodeDeck 的项目扫描交互作为 DevPilot
的主要参考。扫描器必须支持候选项目预览、批量勾选、单独导入和重复项目识别。

## 2.2 Runbox（仓库名 project-launcher）

仓库：https://github.com/siyuhuh/project-launcher

重点借鉴：

-   一个 Project 下配置多个 Task；
-   Task 独立启动、停止和查看日志；
-   进程组管理，停止任务时清理子进程；
-   CPU、内存、运行时间和端口信息；
-   Readiness Check：进程、端口、HTTP、日志关键字；
-   Task Dependencies：先启动依赖，依赖 Ready 后再启动下游任务；
-   Workspace Groups：跨项目组合一套开发环境；
-   桌面应用和 CLI 共享同一份项目、运行状态和日志数据。

特别注意：Runbox 当前 README 明确说明它不会自行递归扫描磁盘，而是提供
Agent 辅助扫描和 CLI 导入流程。因此 DevPilot 应采用 CodeDeck 的 GUI
扫描体验，同时借鉴 Runbox 的任务、就绪检测和工作区模型。

## 2.3 localhost

仓库：https://github.com/marcellosso/localhost

重点借鉴：

-   项目与 Service 分层；
-   本机端口和外部进程发现；
-   服务启动、停止、状态和日志；
-   GUI、CLI、MCP 共享一个后台 Core；
-   MCP 工具采用业务语义，而不是直接暴露任意 Shell；
-   Agent 操作可追踪来源；
-   只读工具和修改工具分离。

DevPilot 应采用 Shared Core
思路：桌面端关闭后，已托管的开发服务仍然可以运行；CLI 和 MCP 连接同一个
Core，看到一致的状态。

## 2.4 借鉴矩阵

  -----------------------------------------------------------------------
  能力                    主要参考                DevPilot 的实现
  ----------------------- ----------------------- -----------------------
  扫描项目目录            CodeDeck                Rust Project Scanner

  项目卡片和导航          Runbox                  中文项目工作台

  端口扫描和进程详情      localhost               macOS Process Discovery

  项目服务配置            CodeDeck / Runbox       Project + Service
                                                  Definition

  启停和日志              Runbox / localhost      Service Supervisor

  就绪检测和依赖          Runbox                  Health Check Engine

  多项目启动              CodeDeck Launch Sets /  Workspace Group
                          Runbox Workspace        

  MCP                     localhost               MCP Server → Shared
                                                  Core

  IDEA Run Configuration  JetBrains Platform SDK  IntelliJ IDEA Plugin
  -----------------------------------------------------------------------

## 2.5 代码复用原则

-   优先复用成熟的算法、数据模型思路、交互流程和测试用例。
-   如需复制代码，必须逐文件检查 LICENSE、版权声明、依赖许可证和来源。
-   CodeDeck 和 Runbox 仓库公开标注 MIT；localhost
    的实际复用范围必须以目标 commit 中的 LICENSE 文件为准。
-   不直接复制整个仓库后再大量改名。
-   在 `THIRD_PARTY_NOTICES.md` 中记录复用代码、来源
    URL、commit、许可证和修改内容。
-   参考项目持续变化，开发前应固定研究 commit，避免把 README
    当前功能误认为已验证的稳定 API。

------------------------------------------------------------------------

# 3. 产品范围与版本规划

## 3.1 V1.0 总体范围

V1.0 以 macOS 单机开发场景为主，分阶段交付。

### P0：第一阶段必须完成

-   项目目录扫描；
-   项目识别和导入；
-   项目列表、搜索、收藏和分类；
-   IDE 快捷打开；
-   本机 TCP 监听端口扫描；
-   进程详情；
-   工作目录与项目归属推断；
-   外部进程识别；
-   安全停止进程；
-   中文 UI；
-   SQLite 持久化。

### P0：第二阶段

-   Node、Python、Java 命令行服务启动；
-   服务停止、重启；
-   stdout / stderr 日志；
-   服务运行状态；
-   端口冲突阻断；
-   就绪检测；
-   进程组清理；
-   托管服务与外部服务区分。

### P1：第三阶段

-   IntelliJ IDEA 插件；
-   IDEA Run Configuration 发现、启动和停止；
-   MCP Server；
-   CLI；
-   Workspace Groups；
-   多服务依赖和顺序启动。

### P2：后续增强

-   菜单栏常驻；
-   全局快捷键；
-   项目 Git 状态；
-   资源占用趋势；
-   配置导入导出；
-   Windows / Linux 适配；
-   更多 IDE 插件。

## 3.2 非目标

V1.0 不做：

-   Docker / Docker Compose 管理；
-   远程服务器和 SSH 进程管理；
-   生产环境部署；
-   云端账号、团队协作和项目同步；
-   完整 Git 客户端；
-   IDE 编辑器功能；
-   自动修改项目端口或源码；
-   自动安装 Node、JDK、Python 等运行环境；
-   未经授权自动终止未知进程。

------------------------------------------------------------------------

# 4. 信息架构与 UI 设计

## 4.1 全局布局

采用 Runbox 的左侧菜单导航风格，使用中文界面、深色主题作为默认主题。

建议主窗口：

``` text
┌──────────────────────────────────────────────────────────────────┐
│ DevPilot                                        搜索   快捷操作   │
├─────────────────┬────────────────────────────────────────────────┤
│ 项目搜索        │                                                │
│                 │              主内容区域                         │
│ 全部项目        │                                                │
│ 已收藏          │                                                │
│ 运行中          │                                                │
│ 端口管理        │                                                │
│ 已停止          │                                                │
│                 │                                                │
│ 工作区组        │                                                │
│                 │                                                │
│                 │                                                │
│ 设置            │                                                │
└─────────────────┴────────────────────────────────────────────────┘
```

左侧导航：

1.  全部项目
2.  已收藏
3.  运行中
4.  端口管理
5.  已停止
6.  工作区组
7.  设置

侧边栏底部：

-   Core 运行状态；
-   CLI 安装状态；
-   MCP 连接状态；
-   当前本机信息；
-   版本信息。

## 4.2 全部项目页面

提供三种视图：

-   卡片视图：适合日常使用；
-   紧凑视图：适合大量项目；
-   表格视图：适合查看路径、技术栈、运行状态等信息。

项目卡片建议显示：

-   项目名称；
-   项目路径；
-   Git 分支（可选）；
-   技术栈标签；
-   IDE 图标；
-   服务数量；
-   当前运行状态；
-   快捷打开；
-   启动 / 停止；
-   更多操作。

项目操作：

-   打开项目目录；
-   在 Finder 中显示；
-   使用默认 IDE 打开；
-   使用其他 IDE 打开；
-   打开终端；
-   编辑项目；
-   添加服务；
-   收藏 / 取消收藏；
-   移除 DevPilot 项目记录（不得删除源目录）。

## 4.3 项目扫描弹窗

借鉴 CodeDeck 截图中的扫描弹窗。

交互步骤：

1.  用户选择基础目录；
2.  执行只读递归扫描；
3.  展示候选项目；
4.  显示识别出的语言、框架、构建工具；
5.  用户勾选需要导入的项目；
6.  批量导入；
7.  对识别不确定的项目标注"需要确认"。

扫描弹窗字段：

  字段       说明
  ---------- -----------------------------------------------
  项目名称   默认使用目录名，可修改
  项目路径   绝对路径
  项目类型   Java / Node.js / Python / Rust / Go / Other
  框架       Spring Boot / React / Vue / Vite / FastAPI 等
  构建工具   Maven / Gradle / npm / pnpm / yarn / uv 等
  Git        是否 Git 仓库
  候选原因   哪些文件使其被识别为项目
  导入状态   未导入 / 已导入 / 重复 / 需要确认

扫描规则：

-   默认不跟随符号链接；
-   支持最大扫描深度配置；
-   跳过 `.git`
    内部目录、`node_modules`、`target`、`dist`、`build`、`coverage`、`.next`、`.nuxt`、`.gradle`、`.venv`、`venv`、`__pycache__`、缓存和备份目录；
-   对 monorepo 识别父级和子项目关系；
-   不重复导入同一路径；
-   不执行任何项目命令；
-   扫描结果必须可预览、可取消；
-   对超大目录显示进度和取消入口。

项目识别规则：

  类型                 识别依据
  -------------------- ----------------------------------------------------
  Node.js              `package.json`
  pnpm                 `pnpm-lock.yaml` / `pnpm-workspace.yaml`
  React / Vue / Vite   package.json dependencies、scripts、配置文件
  Java Maven           `pom.xml`
  Java Gradle          `build.gradle` / `build.gradle.kts`
  Spring Boot          Maven/Gradle 依赖中存在 Spring Boot 相关坐标
  Python               `pyproject.toml` / `requirements.txt` / `setup.py`
  Rust                 `Cargo.toml`
  Go                   `go.mod`

识别不等于确认可运行。比如仅有 `package.json`
的工具库不一定是可启动服务。扫描器应输出"项目类型"和"可运行候选"两种结果。

Monorepo 处理：

-   检测 `pnpm-workspace.yaml`、package.json workspaces、Maven
    modules、Gradle settings；
-   默认优先导入仓库根项目；
-   同时展示可独立运行的子项目；
-   不自动同时导入父项目和所有子目录；
-   由用户选择需要管理的粒度。

## 4.4 项目详情页

项目详情建议包含：

-   基本信息；
-   服务列表；
-   启动任务；
-   项目路径；
-   技术栈；
-   默认 IDE；
-   运行状态；
-   最近运行记录；
-   项目操作。

项目下可以有多个服务，例如：

``` text
pets-care-assistant
├── frontend
│   ├── Type: Node.js
│   ├── Command: pnpm dev
│   └── Port: 5173
├── backend
│   ├── Type: Java
│   ├── Launch: IntelliJ IDEA
│   └── Port: 8080
└── worker
    ├── Type: Python
    └── Command: uv run worker.py
```

## 4.5 端口管理页面

重点借鉴用户提供的 localhost 截图，采用三栏式布局：

``` text
┌──────────────┬──────────────────────┬───────────────────────────┐
│ 左侧导航     │ 监听端口列表         │ 进程详情                  │
│              │                      │                           │
│ 全部项目     │ 3000 node            │ Ports                     │
│ 运行中       │ 5173 node            │ Command                   │
│ 端口管理     │ 8080 java            │ Executable                │
│              │ 8081 java            │ PID / PPID                │
│              │ 8788 python          │ Process Group             │
│              │ 6379 redis           │ Working Directory         │
│              │                      │ Belongs To                │
│              │                      │ Launch Source             │
└──────────────┴──────────────────────┴───────────────────────────┘
```

端口列表每项显示：

-   端口；
-   进程名称；
-   命令摘要；
-   所属项目；
-   启动来源；
-   运行状态；
-   操作按钮。

详情面板显示：

-   监听地址和端口；
-   完整启动命令；
-   Executable；
-   PID；
-   PPID；
-   Process Group ID；
-   Working Directory；
-   项目归属；
-   归属置信度；
-   Launch Source；
-   首次发现时间；
-   运行时长；
-   是否由 DevPilot 托管；
-   是否可以安全停止；
-   关联服务配置。

端口列表应支持：

-   搜索端口；
-   搜索 PID；
-   搜索命令；
-   搜索项目路径；
-   按语言和进程类型筛选；
-   按启动来源筛选；
-   按项目筛选；
-   自动刷新（默认 2 秒，可配置）；
-   手动刷新。

注意：端口扫描只会发现监听端口，不等于发现全部运行进程。没有监听端口的
Java 定时任务、构建任务和后台
Worker，必须通过托管任务记录或进程列表补充展示。

## 4.6 服务详情页

服务详情包含：

-   启动配置；
-   运行状态；
-   端口；
-   PID / 进程组；
-   日志；
-   健康检查；
-   运行时间；
-   最近退出码；
-   启动 / 停止 / 重启；
-   打开浏览器；
-   打开项目；
-   打开 IDE。

服务状态不能只使用 Running / Stopped 两种状态，必须使用状态机（见第 8
章）。

## 4.7 工作区组页面

Workspace Group 用于将多个项目的服务组合起来。

示例：

``` text
工作区：个人产品开发环境

1. Redis
2. MongoDB
3. management-api（Java）
4. management-web（Node）
5. content-worker（Python）
6. 打开浏览器
```

支持：

-   新建工作区；
-   添加项目服务；
-   调整启动顺序；
-   设置依赖关系；
-   设置服务就绪条件；
-   一键启动；
-   一键停止；
-   启动失败时停止本次新启动的服务；
-   查看工作区启动记录。

------------------------------------------------------------------------

# 5. 核心业务流程

## 5.1 扫描并导入项目

``` mermaid
flowchart TD
    A[用户选择基础目录] --> B[只读递归扫描]
    B --> C[识别项目清单和技术栈]
    C --> D[识别 Monorepo 和嵌套项目]
    D --> E[生成候选项目列表]
    E --> F{用户确认}
    F -->|导入| G[写入 Project Registry]
    F -->|取消| H[不修改任何数据]
    G --> I[生成项目档案]
```

## 5.2 启动托管服务

``` mermaid
flowchart TD
    A[用户点击启动] --> B[检查服务配置]
    B --> C[检查依赖服务]
    C --> D[检查端口占用]
    D --> E{端口冲突?}
    E -->|是| F[展示占用进程和归属]
    F --> G[阻止启动并等待用户处理]
    E -->|否| H[创建独立进程组]
    H --> I[启动命令]
    I --> J[采集 stdout/stderr]
    J --> K[执行 Readiness Check]
    K --> L{是否 Ready}
    L -->|是| M[状态变更为 READY]
    L -->|否| N[超时或异常，记录失败]
```

## 5.3 发现外部进程

``` mermaid
flowchart TD
    A[周期性扫描监听端口] --> B[获得 PID 和端口]
    B --> C[获取进程命令和 Executable]
    C --> D[获取 PPID 和进程树]
    D --> E[获取 Working Directory]
    E --> F[匹配已登记项目路径]
    F --> G{匹配置信度}
    G -->|高| H[自动建议项目归属]
    G -->|中| I[显示待确认归属]
    G -->|低| J[显示为外部进程]
    H --> K[用户可修正归属]
    I --> K
```

## 5.4 服务冲突处理

当用户启动服务时，如果端口被其他进程占用：

-   显示端口、PID、命令、工作目录；
-   如果能识别项目，显示项目名称；
-   如果是 DevPilot 托管服务，提供停止或切换到已有服务的选项；
-   如果是 IDEA 启动的服务，提示打开 IDEA 或通过 IDEA 插件停止；
-   如果是外部未知服务，只允许查看和在用户明确确认后停止；
-   不允许默认执行 `kill -9`。

------------------------------------------------------------------------

# 6. 核心领域模型

所有 ID 使用 UUID 或稳定字符串 ID。项目路径需要规范化（canonical
path），但保留用户原始输入路径用于展示。

## 6.1 Project

``` json
{
  "id": "project_uuid",
  "name": "management-api",
  "root_path": "/Users/user/projects/management-api",
  "canonical_path": "/Users/user/projects/management-api",
  "project_type": "JAVA",
  "frameworks": ["Spring Boot"],
  "build_tools": ["Maven"],
  "default_ide": "INTELLIJ_IDEA",
  "is_favorite": false,
  "created_at": "ISO-8601",
  "updated_at": "ISO-8601"
}
```

## 6.2 ServiceDefinition

``` json
{
  "id": "service_uuid",
  "project_id": "project_uuid",
  "name": "backend",
  "runtime_type": "JAVA",
  "launch_mode": "COMMAND",
  "executable": "mvn",
  "args": ["spring-boot:run"],
  "working_directory": "/Users/user/projects/management-api",
  "environment": {
    "SPRING_PROFILES_ACTIVE": "dev"
  },
  "expected_ports": [8080],
  "readiness": {
    "type": "HTTP",
    "url": "http://127.0.0.1:8080/actuator/health",
    "timeout_seconds": 90
  },
  "auto_start_with_project": true
}
```

注意：敏感环境变量不能以明文写入普通 SQLite 字段。V1.0 可先使用 macOS
Keychain 存储敏感值，数据库仅保存 Keychain 引用。

## 6.3 ServiceRun

表示某次实际运行实例：

``` json
{
  "id": "run_uuid",
  "service_id": "service_uuid",
  "launch_source": "DEVPILOT",
  "pid": 12345,
  "ppid": 12340,
  "pgid": 12345,
  "started_at": "ISO-8601",
  "stopped_at": null,
  "exit_code": null,
  "status": "READY",
  "log_path": "/Users/user/Library/Logs/DevPilot/run_uuid.log"
}
```

运行时状态必须通过 Core 实时核对，不能仅凭数据库记录判断进程还活着。

## 6.4 ProcessSnapshot

用于表示扫描到的操作系统进程：

-   PID；
-   PPID；
-   Process Group ID；
-   Executable Path；
-   Command Line；
-   Working Directory；
-   User ID；
-   Start Time；
-   CPU / Memory（后续阶段）；
-   Listening Ports；
-   Parent Chain；
-   Discovery Time。

## 6.5 PortBinding

一个进程可能监听多个端口，一个端口也可能有 IPv4、IPv6 或多个监听地址。

字段：

-   PID；
-   Protocol；
-   Local Address；
-   Port；
-   Family；
-   State；
-   Process Snapshot ID；
-   First Seen；
-   Last Seen。

不能简单把 `PID + Port` 当成唯一的所有网络绑定关系。

## 6.6 WorkspaceGroup

字段：

-   ID；
-   Name；
-   Description；
-   Ordered Entries；
-   Dependency Graph；
-   Created At；
-   Updated At。

每个 Entry 指向一个 ServiceDefinition，而不是复制服务配置。

------------------------------------------------------------------------

# 7. SQLite 数据库设计

建议第一版使用 SQLite，启用 WAL 模式和外键约束。

核心表：

  表                       用途
  ------------------------ ----------------------
  `projects`               项目档案
  `project_tags`           项目标签
  `project_scan_records`   扫描记录
  `service_definitions`    服务配置
  `service_runs`           服务运行历史
  `process_snapshots`      最近发现的进程快照
  `port_bindings`          端口监听信息
  `workspace_groups`       工作区
  `workspace_entries`      工作区任务条目
  `ide_connections`        IDE 插件连接
  `operation_audits`       启动、停止等操作审计
  `app_settings`           应用设置

设计约束：

1.  `projects.canonical_path` 建唯一索引，防止重复导入。
2.  `service_definitions.project_id` 外键关联项目。
3.  `service_runs` 记录每次启动，不覆盖历史运行记录。
4.  `process_snapshots` 是短期快照，可以清理历史。
5.  `port_bindings` 只保存近期扫描结果，不作为永久事实。
6.  删除项目档案时，不得删除项目目录、源码和服务日志。
7.  移除项目记录前，如果有托管服务运行，必须先提示用户。

------------------------------------------------------------------------

# 8. 服务生命周期和状态机

## 8.1 服务状态

``` text
STOPPED
  |
  v
STARTING
  |
  +------> BLOCKED
  |
  +------> CRASHED
  |
  v
RUNNING
  |
  v
READY
  |
  v
STOPPING
  |
  v
STOPPED
```

定义：

  状态       含义
  ---------- ------------------------------
  STOPPED    当前无运行进程
  STARTING   进程已创建，尚未通过就绪检查
  RUNNING    进程存在，但尚未确认服务可用
  READY      就绪检查通过
  STOPPING   正在执行停止流程
  BLOCKED    被端口冲突或依赖条件阻断
  CRASHED    非正常退出
  UNKNOWN    进程归属或状态无法确定

不能将"进程启动成功"直接等同于"服务 Ready"。

## 8.2 三种管理来源

### A. Managed：DevPilot 托管

由 DevPilot Core 启动。

可以准确记录：

-   启动命令；
-   参数；
-   工作目录；
-   环境变量引用；
-   PID；
-   PGID；
-   stdout / stderr；
-   运行时间；
-   退出码。

支持完整启停、重启和日志查看。

### B. IDE Managed：IDE 托管

由 IntelliJ IDEA 插件等 IDE 集成启动。

DevPilot 通过插件获得运行配置 ID、配置名称、项目路径、运行实例状态。

停止和重启优先通过 IDE 插件调用对应 Run Configuration
的执行实例，而不是直接杀掉一个猜测出来的 PID。

### C. Observed：外部发现

由 Terminal、Codex、其他应用或用户手动启动。

DevPilot 只能根据进程、端口、命令行、工作目录和进程树推断归属。

默认允许查看；停止必须确认；没有可靠启动配置时不提供自动重启。

## 8.3 进程启动规范

-   默认使用 executable + args，不把命令拼接成未经处理的 Shell 字符串。
-   支持显式配置 Shell Mode，兼容用户的 zsh、nvm、Homebrew、pnpm 环境。
-   启动前检查项目路径和可执行文件。
-   每个托管任务创建独立进程组。
-   stdout 和 stderr 统一采集，保留时间戳。
-   Core 关闭窗口后，已托管任务不应因 GUI 退出而自动停止。
-   关闭服务时先 SIGTERM，等待配置的宽限时间，再根据用户策略 SIGKILL。
-   只对 DevPilot 自己创建并记录的 PGID 执行进程组终止。
-   外部进程不能仅因端口相同就被视为当前服务。

## 8.4 就绪检查

支持四种检查方式：

  类型         判断
  ------------ --------------------------
  PROCESS      进程存活并通过启动宽限期
  PORT         指定 TCP 端口开始监听
  HTTP         HTTP 健康检查返回成功
  LOG_PHRASE   日志中出现指定关键字

支持：

-   超时时间；
-   检查间隔；
-   成功条件；
-   失败处理；
-   启动依赖；
-   失败回滚。

------------------------------------------------------------------------

# 9. macOS 端口和进程发现实现

这是第一阶段最重要的底层能力。

## 9.1 端口发现

MVP 可以使用 macOS 自带 `lsof`：

``` bash
lsof -nP -iTCP -sTCP:LISTEN
```

用途：

-   获取 TCP 监听端口；
-   获取进程名称；
-   获取 PID；
-   获取监听地址；
-   区分 IPv4 / IPv6。

注意：

-   同一个进程可能有多个监听端口；
-   一个端口可能绑定多个地址；
-   IPv4 和 IPv6 监听不能简单合并；
-   `lsof` 输出可能包含权限不足或进程退出导致的瞬时错误；
-   必须正确解析空格、路径和命令字段，不能依赖固定列宽截断。

## 9.2 进程详情

针对每个 PID，进一步获取：

``` bash
ps -ww -p <PID> -o pid=,ppid=,pgid=,etime=,command=
lsof -a -p <PID> -d cwd -Fn
```

MVP 可先通过 `ps` 和 `lsof` 获取基础信息；后续可使用 macOS `libproc` API
提升效率和结构化程度。

建议封装：

``` text
ProcessDiscovery
├── list_listening_ports()
├── inspect_process(pid)
├── get_process_command(pid)
├── get_process_parent(pid)
├── get_process_group(pid)
├── get_process_working_directory(pid)
├── get_process_executable(pid)
└── build_process_tree(pid)
```

所有系统命令调用都必须设置超时、错误处理和输出大小限制。

## 9.3 项目归属识别

推荐综合以下证据：

1.  Working Directory 是否位于某个已登记项目目录；
2.  Executable 或 Command Line 中是否出现项目路径；
3.  父进程链是否包含 IDE、Terminal、Codex 或 DevPilot；
4.  进程组是否与某个托管 ServiceRun 一致；
5.  是否与 IDEA 插件上报的运行实例匹配。

归属结果：

-   HIGH：工作目录明确位于已登记项目根目录；
-   MEDIUM：命令路径或父进程链匹配；
-   LOW：仅通过进程名称或端口进行猜测；
-   NONE：无法匹配。

只有 HIGH 才可以默认自动关联；MEDIUM 显示建议；LOW 和 NONE
归入外部进程。

允许用户手动指定"这个进程属于哪个项目"，并保存用户修正，但不能把临时 PID
当成长期身份标识。

## 9.4 刷新策略

-   默认每 2 秒刷新监听端口；
-   进程详情按需加载；
-   对同一 PID 做短期缓存；
-   PID 退出后立即清除对应活动快照；
-   UI 显示最近更新时间；
-   支持手动刷新；
-   对扫描错误显示可理解的错误提示；
-   不要求 root 权限，不尝试绕过 macOS 隐私限制。

------------------------------------------------------------------------

# 10. IntelliJ IDEA 插件设计

IDEA 集成是 DevPilot 与普通项目启动器拉开差距的重要功能。

## 10.1 目标

让 DevPilot 可以：

-   连接已打开的 IntelliJ IDEA 项目；
-   获取该项目可用的 Run Configuration 列表；
-   显示配置名称、类型和所属项目；
-   请求 IDEA 启动某个配置；
-   查看插件管理的运行实例；
-   停止指定运行实例；
-   重新运行指定配置；
-   接收运行状态变化。

## 10.2 技术方案

开发独立 IntelliJ Platform Plugin，使用 Kotlin。

重点研究 JetBrains 官方 API：

-   `RunManager`：读取项目运行配置；
-   `RunnerAndConfigurationSettings`：配置实例及其设置；
-   `ExecutionManager` / `ProgramRunnerUtil`：程序化执行；
-   `RunContentDescriptor`：运行窗口描述；
-   `ProcessHandler`：进程生命周期和停止；
-   `Project`：当前 IDEA 项目。

官方文档：

https://plugins.jetbrains.com/docs/intellij/run-configurations.html

## 10.3 通信

优先使用本机 Unix Domain Socket：

``` text
DevPilot Core
     |
     | UDS
     |
IntelliJ IDEA Plugin
```

连接时需要：

-   一次性配对令牌；
-   本机用户权限；
-   请求超时；
-   插件版本；
-   IDEA 项目路径；
-   IDE 实例 ID。

不开放公网端口，不使用无认证 HTTP 服务。

## 10.4 插件接口草案

``` text
ide.list_instances
ide.list_projects
ide.list_run_configurations
ide.start_configuration
ide.stop_execution
ide.get_execution_status
```

运行配置返回值不应包含密码、令牌等敏感字段。

## 10.5 重要边界

-   第一版插件优先管理由插件请求启动的运行实例。
-   已经由用户手动启动的 Run
    Configuration，只有在能够可靠识别时才允许关联。
-   不假设所有运行配置都是 Java Application；可能还有
    Maven、Gradle、Spring Boot、JUnit 等。
-   不通过拼接命令行来模拟 IDEA 启动。
-   不直接修改用户原有 Run Configuration。
-   具体 API 需要在目标 IntelliJ IDEA 版本上验证，插件兼容版本单独管理。

------------------------------------------------------------------------

# 11. MCP 和 CLI 设计

## 11.1 MCP 的作用

MCP 是 AI Agent 调用 DevPilot 能力的标准接口。

例如用户在 Codex 中输入：

> 检查 management-api 项目是否运行，如果没有运行，使用 IDEA 的 dev
> 配置启动；等待健康检查成功后再启动前端。

Codex 可以通过 MCP：

1.  查询项目；
2.  查询服务状态；
3.  检查端口；
4.  调用 IDEA 插件启动配置；
5.  等待健康检查；
6.  启动前端；
7.  查询日志；
8.  汇报结果。

MCP 不负责真正管理进程。它只负责把 AI Agent 的调用转发给 DevPilot Core。

## 11.2 MCP 工具定义

### 查询类工具

  工具                   输入                 输出
  ---------------------- -------------------- ------------
  `list_projects`        搜索、标签           项目列表
  `get_project`          project_id           项目详情
  `list_services`        project_id、状态     服务列表
  `get_service_status`   service_id           实时状态
  `list_ports`           port、process_name   监听端口
  `inspect_process`      pid                  进程详情
  `get_service_logs`     service_id、limit    最近日志
  `list_workspaces`      无                   工作区列表

### 修改类工具

  工具                     输入              输出
  ------------------------ ----------------- ----------------
  `start_service`          service_id        启动结果
  `stop_service`           service_id        停止结果
  `restart_service`        service_id        重启结果
  `start_workspace`        workspace_id      工作区启动结果
  `stop_workspace`         workspace_id      停止结果
  `open_project`           project_id、IDE   打开结果
  `preview_project_scan`   root_path         候选项目
  `import_projects`        candidate_ids     导入结果

MCP 修改类工具必须有权限策略。默认可以要求用户在 GUI
中授权或确认；只读模式不暴露修改工具。

## 11.3 MCP 调用安全

-   MCP Server 只允许本机配置的客户端连接；
-   不暴露公网监听端口；
-   不允许任意 Shell 执行；
-   只允许启动已经登记并授权的 ServiceDefinition；
-   `stop_service` 默认只能停止 DevPilot 托管或 IDE 插件确认的实例；
-   终止外部进程必须额外确认；
-   删除项目配置需要确认；
-   MCP 工具调用写入 `operation_audits`；
-   GUI 显示最近的 Agent 操作记录；
-   可以配置只读模式。

MCP 官方工具规范建议保留用户拒绝调用的能力，并让用户清楚知道暴露给 AI
的工具及其调用行为。

## 11.4 CLI 命令设计

CLI 名称暂定 `devpilot`：

``` bash
# 查看帮助
devpilot --help

# 列出项目
devpilot project list

# 扫描目录（只预览）
devpilot project scan ~/ai_code

# 查询所有服务
devpilot service list

# 查询端口
devpilot ports

# 查看端口占用详情
devpilot ports inspect 8080

# 启动服务
devpilot service start <service-id>

# 停止服务
devpilot service stop <service-id>

# 查看日志
devpilot service logs <service-id>

# 启动工作区
devpilot workspace start <workspace-id>

# MCP Server（stdio）
devpilot mcp serve
```

CLI 与 GUI 共用 Core，不允许 CLI 直接维护第二套 SQLite 状态。

------------------------------------------------------------------------

# 12. 技术架构

## 12.1 技术选型

  ------------------------------------------------------------------------------
  层级                    技术                    说明
  ----------------------- ----------------------- ------------------------------
  Desktop                 Tauri 2                 macOS 桌面应用

  Frontend                React + TypeScript      中文 UI

  UI                      Tailwind CSS +          统一视觉体系
                          shadcn/ui 或同类组件    

  Core                    Rust                    进程、文件系统、服务生命周期

  Database                SQLite                  项目和配置持久化

  IPC                     Unix Domain Socket      GUI / CLI / MCP / IDEA 通信

  CLI                     Rust + clap             终端入口

  MCP                     Rust MCP SDK 或稳定     AI Agent 接入
                          JSON-RPC 实现           

  IDEA Plugin             Kotlin + IntelliJ       Run Configuration 集成
                          Platform SDK            

  Logs                    文件日志 + SQLite 索引  本地日志
  ------------------------------------------------------------------------------

## 12.2 逻辑架构

``` mermaid
flowchart TB
    UI[DevPilot Desktop<br/>Tauri + React]
    CLI[DevPilot CLI]
    MCP[MCP Server]
    IDEA[IntelliJ IDEA Plugin]

    CORE[DevPilot Core / Daemon]
    DB[(SQLite)]
    SCAN[Project Scanner]
    DISC[Process Discovery]
    SUP[Service Supervisor]
    WORK[Workspace Engine]
    IDE[IDE Integration]

    UI <--> CORE
    CLI <--> CORE
    MCP <--> CORE
    IDEA <--> CORE

    CORE <--> DB
    CORE --> SCAN
    CORE --> DISC
    CORE --> SUP
    CORE --> WORK
    CORE --> IDE
```

## 12.3 推荐仓库结构

``` text
devpilot/
├── apps/
│   ├── desktop/                 # Tauri + React 桌面端
│   └── cli/                     # devpilot CLI
├── crates/
│   ├── devpilot-domain/         # 领域模型
│   ├── devpilot-core/           # Core 服务和用例
│   ├── devpilot-storage/        # SQLite
│   ├── devpilot-ipc/            # UDS 协议
│   ├── devpilot-scanner/        # 项目扫描
│   ├── devpilot-process/        # 进程发现和控制
│   ├── devpilot-port/           # 端口扫描
│   ├── devpilot-supervisor/     # 服务生命周期
│   ├── devpilot-workspace/      # 工作区编排
│   ├── devpilot-ide/            # IDE 抽象接口
│   └── devpilot-mcp/            # MCP Server
├── plugins/
│   └── intellij/                # Kotlin IDEA 插件
├── docs/
│   ├── PRD.md
│   ├── ARCHITECTURE.md
│   ├── DATABASE.md
│   ├── IPC-PROTOCOL.md
│   ├── MCP-TOOLS.md
│   └── DEVELOPMENT-PLAN.md
├── scripts/
├── THIRD_PARTY_NOTICES.md
├── README.md
└── LICENSE
```

第一阶段可以先采用较小的 Cargo
workspace，不必为了目录完整而提前创建大量空
crate。模块边界确定后再拆分。

## 12.4 Core 生命周期

建议 Core 作为用户级后台进程：

-   不需要 root 权限；
-   通过 macOS LaunchAgent 实现可选自启动；
-   用户关闭 GUI 时 Core 可以继续运行；
-   Core 退出不应无故终止已托管的服务；
-   GUI 重新打开后从 Core 获取实时状态；
-   Core 使用 UDS 与客户端通信；
-   Socket 文件权限限制为当前用户；
-   Core 异常退出后，启动时重新扫描进程并恢复可识别的运行记录。

注意：Core 的自启动和后台常驻必须由用户明确开启。第一阶段可先实现 GUI
启动时拉起 Core，后续再加入 LaunchAgent 安装流程。

------------------------------------------------------------------------

# 13. 安全设计

## 13.1 项目扫描安全

-   只读扫描；
-   不执行项目脚本；
-   不自动安装依赖；
-   不读取 `.env` 文件内容作为项目元数据；
-   不把源码上传到网络；
-   不自动导入未知 Shell 命令；
-   用户确认后才执行导入的启动配置。

## 13.2 命令执行安全

-   默认使用 executable + args；
-   Shell Mode 必须显式开启；
-   不拼接用户输入形成任意 Shell 命令；
-   执行前显示完整命令、工作目录和环境变量差异；
-   首次执行项目扫描得到的命令必须确认；
-   敏感环境变量不显示在日志中；
-   启动命令的修改需要记录审计信息。

## 13.3 进程终止安全

按安全级别：

1.  DevPilot 自己托管的进程组：允许正常停止；
2.  IDEA 插件确认的运行实例：通过插件停止；
3.  外部已确认归属进程：确认后发送 SIGTERM；
4.  未知进程：只展示，不提供默认快捷终止；
5.  SIGKILL：必须二次确认，并展示 PID、命令和项目归属。

不允许使用 `killall node`、`pkill java` 这种按进程名称批量终止的实现。

## 13.4 MCP 安全

-   只开放业务工具；
-   禁止任意 Shell 工具；
-   查询和修改权限分离；
-   支持只读模式；
-   支持操作确认；
-   所有 Agent 操作留痕；
-   Core 只接受本机授权客户端。

------------------------------------------------------------------------

# 14. 日志、监控和异常处理

## 14.1 日志

每个托管 ServiceRun 一个独立日志文件：

``` text
~/Library/Logs/DevPilot/
├── core.log
├── services/
│   ├── <run-id>.log
│   └── <run-id>.log
└── operations/
    └── audit.log
```

要求：

-   stdout / stderr 合并或标记来源；
-   每行带时间戳；
-   日志支持尾部读取；
-   支持关键词搜索；
-   支持日志大小限制；
-   支持历史日志清理；
-   日志不包含明文密钥。

## 14.2 异常情况

必须处理：

-   进程启动失败；
-   可执行文件不存在；
-   JDK / Node / Python 环境不可用；
-   端口被占用；
-   服务启动超时；
-   服务启动后立即退出；
-   父进程退出但子进程仍存在；
-   PID 被系统复用；
-   用户关闭 GUI；
-   Core 崩溃；
-   IDEA 关闭；
-   IDEA 插件断连；
-   权限不足无法读取某进程详情；
-   用户手动在外部终止服务。

PID 不是永久身份。运行记录至少应同时校验
PID、进程启动时间和命令/路径，避免把系统复用后的相同 PID 误判为原服务。

------------------------------------------------------------------------

# 15. 测试策略与验收标准

## 15.1 第一阶段验收

### 项目扫描

-   能扫描指定目录；
-   能识别 Java Maven / Gradle；
-   能识别 Node npm / pnpm；
-   能识别 Python；
-   能识别 monorepo；
-   能跳过 node_modules、target、dist 等目录；
-   不重复导入同一路径；
-   不执行任何项目命令；
-   用户取消后不产生导入记录。

### 端口扫描

-   能发现 TCP IPv4 / IPv6 监听；
-   能展示 PID、PPID、命令；
-   能获取可访问的工作目录；
-   能识别同一进程的多个端口；
-   能识别进程退出；
-   能处理权限不足；
-   默认刷新周期约 2 秒；
-   支持手动刷新。

### 项目关联

-   工作目录在项目根目录内时可以高置信度关联；
-   命令路径匹配时提供关联建议；
-   无法识别时显示外部进程；
-   用户可以手动修正归属。

### IDE 打开

-   可以使用 IntelliJ IDEA 打开 Java 项目；
-   可以使用 VS Code / Cursor 打开 Node/Python 项目；
-   IDE 未安装或 CLI 不可用时显示错误并允许配置应用路径；
-   不允许打开失败后静默忽略。

## 15.2 第二阶段验收

-   Node 服务能启动、停止、重启；
-   Python 服务能启动、停止、重启；
-   Java Maven / Gradle 命令能启动；
-   日志实时可见；
-   端口冲突会阻止重复启动；
-   服务 Ready 状态来自实际检查；
-   停止后子进程不残留；
-   GUI 关闭后托管服务可继续运行；
-   Core 重启后可以重新识别运行状态。

## 15.3 第三阶段验收

-   IDEA 插件可以列出 Run Configuration；
-   可以通过 DevPilot 启动指定配置；
-   可以通过 DevPilot 停止插件启动的实例；
-   IDEA 关闭后状态能更新；
-   Codex 可以通过 MCP 查询项目和服务；
-   Codex 可以启动、停止已授权服务；
-   MCP 与 GUI 显示的状态一致；
-   所有修改操作有审计记录。

------------------------------------------------------------------------

# 16. 分阶段开发任务

## Phase 0：工程初始化

-   创建 Git 仓库；
-   初始化 Tauri 2 + React + TypeScript；
-   初始化 Rust workspace；
-   配置 SQLite；
-   配置统一代码规范；
-   搭建中文 UI 框架；
-   建立测试和 CI。

## Phase 1：项目管理与进程发现

建议按以下顺序：

1.  左侧菜单和主布局；
2.  项目列表；
3.  项目扫描弹窗；
4.  项目识别器；
5.  项目导入与持久化；
6.  IDE 快捷打开；
7.  macOS TCP 监听扫描；
8.  进程详情；
9.  工作目录识别；
10. 项目归属推断；
11. 进程安全停止；
12. 验收测试。

## Phase 2：Service Supervisor

1.  ServiceDefinition 配置；
2.  命令和参数编辑；
3.  环境变量管理；
4.  进程组创建；
5.  stdout / stderr 采集；
6.  启动、停止、重启；
7.  端口冲突；
8.  Readiness Check；
9.  日志页面；
10. 运行历史；
11. 异常恢复。

## Phase 3：Workspace + CLI + MCP

1.  Workspace Group；
2.  Task Dependency；
3.  失败回滚；
4.  CLI；
5.  MCP Server；
6.  Codex 配置文档；
7.  Agent 操作审计；
8.  只读和写入权限策略。

## Phase 4：IDEA 插件

1.  初始化 IntelliJ Platform Plugin；
2.  获取当前 IDEA 项目信息；
3.  枚举 Run Configuration；
4.  建立 UDS 连接；
5.  插件配对和认证；
6.  远程请求启动配置；
7.  跟踪运行实例；
8.  停止和重启；
9.  运行状态同步；
10. IDEA 版本兼容测试。

------------------------------------------------------------------------

# 17. Codex 开发执行提示词

下面的提示词可以在 DevPilot 仓库根目录中使用。

``` text
你现在负责开发 DevPilot，一个 macOS 本地开发项目与服务管理工具。

首先完整阅读 docs/PRD.md（本文件）以及 docs/ 下的其他设计文档。

必须遵循：

1. 先研究参考项目：
   - https://github.com/JadnK/CodeDeck
   - https://github.com/marcellosso/localhost
   - https://github.com/siyuhuh/project-launcher

2. 不要直接复制三个项目并拼接。
   分别研究项目扫描、端口扫描、进程管理、任务生命周期、
   Workspace、Shared Core 的实现，记录可借鉴点。

3. 采用 Tauri 2 + React + TypeScript + Rust。
   先做 macOS，不使用 Docker。

4. 所有 GUI、CLI、MCP 后续必须共用 Rust Core。
   不允许在前端单独维护一份进程运行状态。

5. 第一轮只实现 Phase 0 和 Phase 1。
   不要提前开发 MCP、IDEA 插件、Workspace 等后续功能。

6. 第一阶段必须完成：
   - 中文项目管理界面；
   - 项目目录扫描；
   - Java、Node、Python 项目识别；
   - 项目导入和 SQLite 持久化；
   - IntelliJ IDEA、VS Code、Cursor 快捷打开；
   - TCP 监听端口扫描；
   - PID、PPID、命令、Executable、Working Directory；
   - 进程详情；
   - 项目归属推断；
   - 外部进程识别；
   - 安全停止进程。

7. 扫描只读，不执行项目脚本。
   不允许自动安装依赖或修改项目文件。

8. 不允许自动终止未知进程。
   进程归属不确定时必须标记为待确认。

9. UI 必须参考：
   - localhost 的三栏 Ports 页面；
   - CodeDeck 的 Scan project folder 弹窗；
   - Runbox 的中文左侧菜单和项目工作台。

10. 每完成一个模块：
    - 运行测试；
    - 运行 cargo check；
    - 运行前端构建；
    - 修复错误；
    - 更新 docs/DEVELOPMENT-PLAN.md；
    - 提交可验证的验收结果。

11. 不允许用静态假数据伪装系统进程扫描结果。
    如果某项 macOS 原生能力暂时无法实现，明确报告限制。

现在先进行仓库初始化和技术调研，输出：
- 现有仓库结构；
- 参考项目源码研究报告；
- Phase 1 技术任务拆分；
- 数据库初始模型；
- 风险清单。

得到这些结果后再开始 Phase 1 编码。
```

------------------------------------------------------------------------

# 18. 参考资料

## 开源项目

1.  CodeDeck\
    https://github.com/JadnK/CodeDeck

2.  localhost\
    https://github.com/marcellosso/localhost

3.  Runbox / project-launcher\
    https://github.com/siyuhuh/project-launcher

## 官方技术文档

1.  Tauri 2\
    https://v2.tauri.app/

2.  IntelliJ Platform Plugin SDK - Run Configurations\
    https://plugins.jetbrains.com/docs/intellij/run-configurations.html

3.  Model Context Protocol - Tools\
    https://modelcontextprotocol.io/specification/2025-06-18/server/tools

4.  Rust\
    https://www.rust-lang.org/

------------------------------------------------------------------------

# 19. 最终产品验收愿景

DevPilot 最终应该让日常开发流程变成：

1.  打开 DevPilot；
2.  在项目列表找到项目；
3.  点击"打开 IDEA"或"打开 VS Code"；
4.  点击项目服务的"启动"；
5.  查看前后端服务状态、日志和端口；
6.  Codex 可以通过 MCP 查询和管理同一批服务；
7.  项目不需要时，一键停止；
8.  端口冲突时，准确知道哪个项目、哪个进程正在占用。

开发者不再需要记住项目路径、寻找终端标签、重复输入启动命令，也不再因为
Codex 留下的服务而反复遇到端口冲突。

**DevPilot 的核心价值不是替代 IDE，而是成为 IDE、项目、服务、进程和 AI
Agent 之间的统一控制中心。**
