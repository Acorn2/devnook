# 本机服务与原生客户端的扫描边界

2026-11-13。用户已授权排除纯 iOS 客户端，并补充其他同类工程，保留服务端、混合仓库后端及移动 / 桌面工程中的独立开发服务器。

## 本轮规则

| 高置信度客户端线索 | 默认处理 | 独立服务入口的例外 |
| --- | --- | --- |
| Xcode 工程、Swift Package 中明确的移动平台 | 不作为客户端候选 / 服务展示 | Swift 可执行目标 + Vapor / Hummingbird 等服务端依赖；同仓库其他后端 |
| Android application/library 插件或 Manifest、JavaFX / Compose UI | 排除原生目标；只有客户端的 Gradle 聚合根也排除 | Spring Boot / Ktor / 明确后台入口；独立后端模块 |
| Flutter SDK + 应用入口 | 不凭 lib/main.dart 标为服务 | 明确的 flutter run -d web-server 脚本；Dart 服务端入口 |
| .NET WPF / WinForms / MAUI / WinUI、移动平台目标 | 排除 GUI / 移动应用与纯客户端 solution 聚合根 | ASP.NET Core / Worker SDK / 独立 Console 后台模块 |
| Electron / Tauri、Rust GUI 框架、Qt Widgets/Quick 等 | 排除客户端启动命令和原生组件 | 单独的 Vite / Next 等 Web 服务脚本，或独立 Rust / C++ 后端 |
| Python GUI 依赖 / gui-scripts | 排除明确 GUI 入口 | FastAPI / Django / Celery 等服务或明确 CLI / 后台入口 |
| Go Fyne / Gio / Ebiten | 不把明确 GUI / 游戏入口作为服务 | 独立的普通 Go CLI 或后端模块 |
| Unity / Unreal / Godot 工程标记 | 不把游戏或编辑器工程作为服务 | 仓库中独立后端；自定义 headless 场景不自动推断 |
| Expo / React Native | 原生构建 / run-ios 不当服务 | 保留 expo start / react-native start，禁止把打开模拟器的脚本当默认服务入口 |

这些是只读静态规则，不能保证识别任意自定义启动器。未知工程不因语言、目录名、没有端口或仅有 main 文件而被当成原生客户端。普通 CLI / worker、库组件沿用既有发现能力；库不凭空生成启动命令。

## 实现边界

- Core 统一判定入口资格；项目识别、分组和服务启动建议复用规则，避免扫描保留 Web 入口但服务页仍建议 electron / tauri / run-ios。
- 每个路径先过滤其客户端语言 / 目标，保留明确的同路径服务证据。独立嵌套 Git 边界仍优先，父级客户端不能吞掉子级后端，子级客户端也不能使父级正常服务被排除。
- 分组后排除只有客户端与无服务证据的聚合根；混合仓库保留服务成员及相关库，不让已过滤客户端影响类型、服务数或默认勾选。独立嵌套 Git 仓库按自身线索判断；不因目录名为 backend/api/server 就认定其为独立服务。
- 默认和深度补扫使用同一资格规则。补扫只扩大目录覆盖；沿用符号链接、.env、清单大小和遍历预算保护。
- 既有项目 / 服务配置不做迁移、删除或停止；service_drafts 只提供合格建议，不覆盖已保存服务。
- 不新增依赖和数据库 schema。UDS 升到 v9，确保新版不静默连接 v8 的旧规则；不退出用户旧 Core。

## 验收

1. 纯 iOS、Android、Flutter、.NET GUI、桌面 / 游戏 / Python GUI 无候选，补扫也不将其变成服务。
2. Swift 服务端、React Native / Expo、Flutter web-server、普通 CLI / worker 保留；脚本选择与预览一致。
3. 相同目录和分模块的客户端 + 后端保留后端；同类型 Android + Spring Boot、WPF + ASP.NET 也覆盖。
4. 独立嵌套仓库、已有偏好 / 服务配置、库组件、无端口的后台程序不误删。
5. 客户端线索在注释或符号链接中的反例不触发排除；扫描及服务建议不执行工程命令。
6. 全量 Rust 回归、TS / 格式 / 构建、隔离 Core GUI 预览、包内 Core、签名和固定路径旧包核对；记录实际结果。

实施验收：新增 8 项资格测试，全量 59 项通过；默认与 8 层补扫都排除隔离样例的 6 类纯客户端根，保留 9 个服务项目。React 预览导入、启动建议、桌面二进制与已签名包内 Core 均已验证。后续原生启动修复新增 2 项测试，全量为 61 项通过；经用户授权正常停止旧后台后，新包已移至标准输出并清理旧包。实际进度见 [DEVELOPMENT-PLAN.md](DEVELOPMENT-PLAN.md)。

## 参考依据

- [Android 构建配置](https://developer.android.com/build)：Android 插件标识属于构建目标线索。
- [.NET Desktop SDK 属性](https://learn.microsoft.com/en-us/dotnet/core/project-sdk/msbuild-props-desktop)：UseWPF / UseWindowsForms 等是 GUI 证据。
- [Flutter Web](https://docs.flutter.dev/platform-integration/web/setup)：web-server 模式可独立提供本机开发服务器。
- [Swift 服务端](https://www.swift.org/documentation/server/guides/building.html)、[Expo CLI](https://docs.expo.dev/more/expo-cli/)：不能按 Swift 或移动项目语言一刀切。
