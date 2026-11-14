# 项目特征扫描设计与实施计划

日期：2026-11-13。用户已授权先规划类型范围，再实施、验证、打包和清理旧包。

后续收敛：11 类清单识别继续用于发现证据，但明确纯原生客户端不再作为服务项目候选。默认与补扫统一的资格、混合仓库及启动建议规则见 [service-eligibility-plan.md](service-eligibility-plan.md)；下文 v8 验收保留为本阶段历史记录。

## 范围与验收

默认按项目清单和模块声明发现项目，识别项目后不继续无差别递归源码、素材和文档。保留前后端、共享库和多模块证据，独立嵌套仓库单列；扫描 / 导入不创建或启动服务。不是通过提高十万条目上限解决问题。

| 工程种类 | 项目特征 | 模块定位 / 边界 |
| --- | --- | --- |
| JavaScript / TypeScript | package.json；React / Vue / Next / Nuxt / Vite / Express / Nest 等依赖线索 | npm / Yarn workspaces、pnpm packages（含排除）、常见前后端与 apps / packages 布局 |
| Java / Kotlin | pom.xml、build.gradle(.kts)、settings.gradle(.kts)；Spring Boot / application 特征 | Maven modules；Gradle 字面量 include / includeBuild 与 projectDir 映射；动态脚本不执行 |
| Python | pyproject.toml、requirements.txt、setup.py、setup.cfg、Pipfile、manage.py | uv workspace members / exclude；Django / FastAPI / Flask 等为框架线索，依赖本身不保证可启动 |
| Rust | Cargo.toml；根二进制入口线索 | workspace members / exclude 与仓库内 path 依赖；只读解析 TOML |
| Go | go.mod、go.work | go.work use 中的本地模块；常见 cmd 布局只定位项目，不猜测服务 |
| .NET | .sln / .slnx、.csproj / .fsproj / .vbproj | solution 引用与仓库内 ProjectReference；ASP.NET Web SDK / OutputType 作为入口线索 |
| PHP | composer.json；Laravel / Symfony 依赖与 artisan | 常见子应用目录；库与应用分开标记 |
| Ruby | Gemfile、*.gemspec、Rails config/application.rb | 常见子应用目录；Gemfile 不执行 |
| Dart / Flutter | pubspec.yaml；Flutter SDK 线索 | pubspec workspace 路径；lib/main.dart 只做定点存在性检查 |
| Swift | Package.swift、*.xcodeproj / *.xcworkspace | 工程根识别；Swift 配置不求值、不递归 Sources；不承诺解析任意 Xcode 配置 |
| C / C++ | CMakeLists.txt、meson.build | 字面量 add_subdirectory / subdir；变量 / 条件不求值 |

此次扩大的是发现覆盖范围，不扩展执行引擎或凭空生成所有语言的启动命令。未能可靠确定入口的工程保留“需确认”或手动填写命令；没有清单的任意脚本、动态生成模块、任意 Makefile / Docker 编排暂不猜测为独立项目。

## 数据和算法

1. depth=0 继续是默认入口，改为特征扫描。未识别项目的集合目录保持有安全深度的发现，普通源码 / 文档 / 素材目录默认剪枝。
2. 每个目录只枚举自身一层并复用结果；定点读取已知清单（单文件仍不超过 1 MiB），不打开普通业务文件。识别项目或仓库后，仅继续声明模块、常见组件容器及非源码子目录中的有限浅探；新识别的子项目重新收敛。
3. 模块展开支持安全相对路径与常用 glob（含排除）。路径必须留在所选扫描根内，逐段拒绝符号链接、依赖 / 构建 / 缓存 / .env 路径。绝对路径、逃逸路径和动态表达式不求值并提示。
4. Git 边界优先于父项目合并；.gitmodules 中的本地 path 定点探查，普通嵌套仓库在常见 / 浅探位置发现。藏在被剪枝源码 / 素材目录深处、且未声明的独立仓库需要深度补扫；默认扫描不声称全盘完整。
5. depth=1–12 沿用原有受限递归，UI 明确为“深度补扫”。它仍跳过依赖、构建和缓存，不跟随符号链接。默认无需开启。
6. 保留 100000 枚举条目、120 秒遍历、5000 原始候选、自动 32 层和补扫 1–12 层保护；路径模式展开共享同一取消、时间、条目和队列预算，不能绕过限制。
7. 候选沿用现有 JSON payload、分组和导入偏好。前端只展示类型 / 模式 / 实际计数和说明。无数据库迁移。Core 协议版本根据是否有不兼容字段变化决定；不终止用户旧 Core。

## 依赖与参考

复用已锁定的 TOML / XML 解析器作为 Core 直接依赖，避免用字符串拆分解析多行 TOML 或把 XML 注释当模块；不安装全局依赖、不运行项目工具。YAML / Gradle / Go / CMake 仅支持说明过的静态字面量子集，复杂语法提示使用补扫或选取具体目录。

- [Cargo workspace members / exclude](https://doc.rust-lang.org/cargo/reference/workspaces.html)：支持成员 glob 和排除。
- [uv workspace](https://docs.astral.sh/uv/concepts/projects/workspaces/)：读取 tool.uv.workspace members / exclude。
- [Maven 多模块](https://maven.apache.org/guides/mini/guide-multiple-modules.html)：按模块声明定位。
- [Gradle multi-project](https://docs.gradle.org/current/userguide/multi_project_builds.html)：只读取静态 include / projectDir，不执行 settings。
- [pnpm settings](https://pnpm.io/settings)：优先 pnpm-workspace.yaml 的 packages 与排除项。
- [Go modules](https://go.dev/ref/mod#workspaces)：只读 go.work use。
- [Dart workspace](https://dart.dev/tools/pub/workspaces)：pubspec workspace。

## 增量任务

### 1. 类型和声明识别

文件：scanner.rs、scanner 内特征 / 声明模块、types.ts、components.tsx。

验证：11 类最小清单 fixture；依赖文件 / 共享库不冒充确定可启动服务；Maven 注释、TOML 多行、Gradle 映射、solution 引用、glob 排除、非法路径和符号链接。

### 2. 遍历收敛与补扫

文件：scanner.rs、scanner/discovery.rs、ScanModal.tsx、CLI / MCP 文案。

验证：大量 src / assets 子树不被默认展开；非常规深层项目可补扫；前后端配对、无根清单仓库、独立嵌套仓库、重复导入 / 偏好保留、取消和安全上限均回归。记录真实 visited / 枚举计数对比，避免只比较不稳定耗时。

### 3. 集成、文档与打包

文件：README、MVP、ARCHITECTURE、TECH-STACK、AGENTS、DEVELOPMENT-PLAN。

验证：全量 Rust 测试、workspace check、TypeScript、格式与前端构建；隔离 Core + GUI 预览检查默认 / 补扫 / 预览导入；新 .app 签名及包内 Core 扫描。只读盘点用户 ai_code 可用于性能验收，不导入真实项目库。

新包使用固定输出路径。验证通过后核对旧产物字面量路径与进程，再清理未运行的旧包；不终止用户应用、服务或 Core。无提交 / 推送 / 发布。

## 风险与回退

- 静态规则无法解释任意可执行配置：提示局限并保留补扫，不运行 Gradle / Python / Ruby / Swift / CMake 求值。
- 精简发现可能遗漏非标准嵌套目录：明确模式说明，补扫或选择具体目录；原有档案不会因漏扫而删除。
- 声明 glob 可能重新指向大源码树：与正常遍历共享预算和剪枝，明确声明的路径可定点进入但不能进入依赖 / symlink / 所选根外。
- 范围受扫描相关模块约束；分组 / 进程控制 / 已有配置保持原语义。保留原递归作为补扫与回退路径，不迁移或重写项目库。

执行结果在 DEVELOPMENT-PLAN 记录；本文件先固定设计范围，不将未验收能力写入 README。

实施完成：以上三个增量已验收。最终全量 51 项测试通过，真实 ai_code 枚举从上限 100000 降至 26230，包内 Core v8 扫描与持久化验证通过；动态配置和隐藏深层目录限制保留并说明。完整证据见 [DEVELOPMENT-PLAN](DEVELOPMENT-PLAN.md)。
