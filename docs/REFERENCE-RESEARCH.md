# 参考项目研究

研究日期：2026-11-12。commit 通过 `git ls-remote HEAD` 固定，源码与 LICENSE 通过对应 commit 的 raw URL 读取。GitHub REST API 在本环境返回 403，未依赖不可用的 API 推断源码。

## CodeDeck

commit：`74bde66834b00e9713fe5b3f5494c1c403fac941`，MIT。

读取：[commands/projects.rs](https://github.com/JadnK/CodeDeck/blob/74bde66834b00e9713fe5b3f5494c1c403fac941/src-tauri/src/commands/projects.rs)、`src-tauri/src/lib.rs`、LICENSE。

`scan_projects` 使用不跟随链接的目录遍历、深度限制、marker 检查、候选数量限制和排序去重。值得保留的是先生成候选、再显式导入的交互边界。DevNook 重新实现后台扫描会话、取消/进度、规范路径去重和父子粒度选择，没有复制该函数。源码中 200 个候选的固定限制未直接套用；本实现以条目、时间和候选总量限制保护大目录。

## Runbox / project-launcher

commit：`efc07175724e1a6a70c0f823112b1ea4fa9a7ada`，MIT。

读取：[core.rs](https://github.com/siyuhuh/project-launcher/blob/efc07175724e1a6a70c0f823112b1ea4fa9a7ada/src-tauri/src/core.rs)、`src-tauri/src/lib.rs`、README、LICENSE。

运行模型将项目和 Task 分开。启动处使用独立进程组；readiness 等待明确区分进程存活和端口/HTTP/日志就绪，并有超时和失败状态。README 的项目发现流程通过 Agent 检查目录，再使用 CLI 导入，不能当作 GUI 递归扫描的已有实现。本轮只借鉴项目工作台与职责分离；进程组、托管和就绪逻辑留在 Phase 2，未复制源码或提前展示启动成功。

## localhost

commit：`151b8f8fa621e40b178d332f96a826232e3b1dff`，实际读取 LICENSE 确认为 MIT。

读取：[core workspace](https://github.com/marcellosso/localhost/blob/151b8f8fa621e40b178d332f96a826232e3b1dff/core/Cargo.toml)、[Core lib](https://github.com/marcellosso/localhost/blob/151b8f8fa621e40b178d332f96a826232e3b1dff/core/localhost-core/src/lib.rs)、[listener.rs](https://github.com/marcellosso/localhost/blob/151b8f8fa621e40b178d332f96a826232e3b1dff/core/localhost-core/src/listener.rs)、README、LICENSE。

workspace 将 Core、CLI、daemon、FFI 分开，Core 不依赖 UI。监听模型包含启动时间、cwd、归属类型和不可停止原因，区分 Run / Project / Unmanaged，说明“有端口”与“由应用托管”是两个维度。DevNook 采用独立 Rust Core，并明确全部 Phase 1 监听为 Observed；停止使用自己的身份凭证流程，不复制自动升级到强制信号的策略。daemon 在存在托管进程前暂不引入。

## 官方资料

- [Tauri 2](https://v2.tauri.app/start/)：桌面壳、前后端边界。
- [Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/)：命令参数、State 和异步调用。
- 当前 Rust libc 源码中的 macOS `proc_bsdinfo` / `proc_pidinfo` 用于核对微秒级进程启动身份。

## 复用边界

未引入上述项目的源码、UI 素材、图标、截图或品牌资源。参考只用于产品交互和工程结构比较。DevNook 图标和样式为本项目自建；第三方依赖由 Cargo.lock / pnpm-lock.yaml 固定。

## 官网信息结构参考（2026-11-14）

本轮按用户指定查看 [localhost 仓库首页](https://github.com/marcellosso/localhost)与 [getlocalhost.app](https://www.getlocalhost.app/)。参考其用途介绍、产品演示、功能场景、安装入口和 FAQ 的信息顺序。DevNook 官网独立实现米白 / 深蓝视觉、静态双语页面、自有界面截图和依据实际发布状态显示的获取入口；不复制对方文案、源码或视觉资产，也不声称 DevNook 已支持对方的 Windows / Linux、远程机器或其他未实现功能。
