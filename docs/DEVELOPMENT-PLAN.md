# DevNook 实施记录

## Intel Mac 发行与有条件的后续计划（2026-11-14）

用户确认本次补齐 Intel DMG 并验证；未来 Windows 等较大投入根据开源项目的实际采用、反馈和维护资源评估。

- 在项目隔离 Rust 工具链中安装 x86_64 标准库，使用已有 Clang / SDK 和 Tauri 构建独立 Intel Release。固定路径为 `target/x86_64-apple-darwin/release/bundle/`，DMG 沿用 Tauri 的 `_x64.dmg` 名称，不覆盖现有 arm64 包。
- Intel `.app` 为 15,962,779 字节，DMG 为 5,182,582 字节（5.18 MB），SHA-256 为 `f1a31777918ed089878d95c13909e05bbe3f08e6e10389ca797ff2fb60c136ad`；macOS 12.0 / x86_64 / ad-hoc / 未公证。
- 发布清单改为同版本的逐架构 builds；独立纯函数校验版本、架构、唯一性、签名与下载文件名，登记候选保留另一架构的公开事实。官网复用原下载区样式，分别展示 Apple Silicon / Intel Mac 的体积、下载和 SHA-256；未公开构建保持源码入口。
- 向既有 v0.1.0 追加 Intel DMG 和 `SHA256SUMS-x86_64`，公开下载逐字节摘要与本地及 GitHub digest 一致。原 ARM 资产的 ID / 大小 / 摘要完全不变，标签不改写；构建时 apps / Core / CLI 业务源码与既有 v0.1.0 一致，前端构建记录仍为 `index-CmHPNo5Y.js`。
- 更新中英文 README 的双架构入口与候选计划：Windows、Linux、更深 IDE 集成和 macOS 发行兼容性。计划无固定排期，不设置 Star 阈值，依据实际采用、可复现反馈、社区贡献和维护资源决定。两版原有 7 个命令代码块保持，链接和锚点检查通过。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| Intel 构建 | `MACOSX_DEPLOYMENT_TARGET=12.0 pnpm tauri build --target x86_64-apple-darwin --no-bundle` 及独立 app / DMG bundle 成功，无新增业务依赖或协议 / 数据库变更 |
| 包完整性 | `lipo` / `file` 确认为 x86_64，`vtool` 最低系统 12.0；codesign strict / deep 和 hdiutil verify 通过。只读挂载后主程序、Info.plist、ICNS 与固定输出完全一致，Applications 链接存在，镜像正常卸载 |
| x86_64 全量测试 | Rosetta 下 `cargo test --release --workspace --target x86_64-apple-darwin --locked`：79 passed、0 failed、5 ignored（内部 fixture） |
| 首次失败与复核 | 首次与打包并行时，子进程 PID 文件读取存在启动竞态，改为等待实际标记并保留身份 / 停止断言，未修改业务就绪定义。Python 项首次在测试配置的 4 秒内未就绪；独立复核和完整回归通过，未调整生产或测试超时。ARM 上对应子进程测试也通过 |
| 原生窗口 | localhost 新增不参与 Start All 的 `intel-desktop-qa`。确认 Intel GUI PID 13588、窗口 131677 为 1280×800；screenshot 回读已有项目和后台已连接，仅作只读检查，随后只停止本轮 GUI |
| 数据目录修正 | Release GUI 不采用 DEVNOOK_DATA_DIR，该变量仅 Debug GUI 生效。原先“GUI 已隔离”的假设已纠正，正式窗口只读连接原 Core；未导入、保存配置或操作用户服务。业务回归全部使用临时 Core / CLI 数据 |
| 站点与记录 | `pnpm site:build` / `pnpm site:check` 通过，8 项发布测试和 66 个站点引用有效；覆盖双架构、混合公开状态、错误链接、重复架构和候选合并。Rust 格式、修改文档及站点的 Prettier、git diff 空白检查通过 |
| 响应式与主题 | 中英 × 浅暗 × 320 / 375 / 414 / 768 / 1024 / 1280 / 1440px 共 28 项通过。展开两个 SHA-256 后仍无横向溢出，两条下载链接准确、按钮高度至少 44px、截图解码正常；桌面和手机的四张代表性渲染已回读。首次脚本等待未进入视口的懒加载图导致超时，改为仅在验收页面触发 eager 后通过，不修改产品懒加载策略 |

Core 65870、runner 95832、服务 95836 / 95880 的 PID / PGID / 启动时间和路径在本轮原生检查后不变，既有 site-preview 保持。Intel 硬件、旧 macOS 真机与 Apple 公证仍未验收，不用 Rosetta 结果替代。原生截图仅保留在本机临时目录，不进入公开截图资源。

工作区出现另一路服务页界面改动，本轮不覆盖或提交这些内容；Intel 包在其出现前已构建。检查日志、公开资产副本和网页验收记录位于 `.tmp/intel-macos-20261114/`。线上部署结果完成后补记。

## v0.1.0 Release 与官网公开发布（2026-11-14）

用户明确授权通过 GitHub CLI / API 部署官网、配置 About / Topics，并发布 v0.1.0、上传最新 DMG 与校验文件；保留现有后台和服务。

- 从 GitHub 官方 Release 下载 arm64 CLI 2.104.0 至忽略目录，校验官方 SHA-256 后使用。设备登录由用户在 Chrome 完成，确认账号为 Acorn2；认证由系统凭据存储维护，未将 Token 写入项目或 hosts 配置。
- 保存英文仓库介绍和 13 个符合已实现能力的标签，不标注尚未支持的 Windows / Linux。官网链接在部署并确认可访问后再写入 About。
- 创建 annotated tag `v0.1.0`，指向通过远端 CI 的源码 `4225ba51f0f91274f9a62e863c0aac40a74e5bb0`。发布 `DevNook v0.1.0` 并标为 Latest，附中英文说明；上传固定输出 DMG 与 SHA256SUMS，未重打包或覆盖正在使用的应用。
- `.app` 14,911,803 字节，DMG 4,695,106 字节（4.70 MB），Apple Silicon / macOS 12+，ad-hoc 签名、未 Developer ID 签名或 Apple 公证。发布说明如实记录平台、签名、校验与使用入口，不将编译模式等同于公证。
- 公开资产重新下载后确认大小、字节摘要与本地文件一致，GitHub 资产摘要也一致；SHA-256 为 `307618b4cdc418ab43d8dd818eb1205c8d69d25d6cce397e430f0e15ed9a2cc7`。随后更新 `site/release.json` 的公开状态、两版 README 下载入口与分发说明。
- GitHub Pages 已通过 API 启用 `workflow` 构建，HTTPS 强制开启。发布配置进入 main 后手动运行工作流，保留后续仅手动发布的规则。

### 实际验证

- 既有源码的 GitHub CI `43587477959` 已成功，包含 macOS 检查、测试、CLI 与原生构建。
- 最新本地 `.app` 的 `codesign --verify --strict --deep`、DMG 的 `hdiutil verify` 通过，Info.plist 为版本 0.1.0 / 最低系统 12.0；DMG 与 SHA256SUMS 的公开下载及 GitHub digest 全部核对一致。
- 公开状态更新后 `pnpm site:build`、`pnpm site:check`（3 项发布状态测试、66 个站点引用）和修改文档 / 清单的 Prettier 检查通过；两语言页面均包含准确 DMG 地址与 SHA-256，无未发布提示。README 原有命令代码块逐字保持，60 处本地文件引用有效，`git diff --check` 通过。
- 官网部署工作流 [43589081590](https://github.com/Acorn2/devnook/actions/runs/43589081590) 的 build / deploy 均成功，部署源码为 `ec5ba5293b4c694c2d6a899aab51f7453559f5cc`。中文首页与 `/en/` 均返回 HTTP 200，HTML 与本地生成结果逐字节一致；20 张功能截图、Logo、CSS 和两个 JS 共 24 项静态资源全部返回 HTTP 200。
- 官网可访问后，保存并读回 About Website `https://acorn2.github.io/devnook/`、实际介绍和 13 个 Topics。Release latest 接口返回 v0.1.0、非草稿 / 非预发布及两个资产；双语 README 增加已上线官网入口。
- 比较任务前后只读进程信息：Core 65870、runner 95832、服务 95836 / 95880 的 PID / PGID / 启动时间与可执行路径一致。本轮不修改业务实现、CI/CD 文件或本机服务配置，不停止、重启现有后台或服务。
- 检查记录和临时 CLI 位于 `.tmp/github-release-20261114/`，不进入提交。线上采用 CLI / API / HTTP 验证，未再调用已停止的电脑操作会话；没有重复浏览器布局验收，旧 macOS / Intel 与公证状态仍遵循发布说明的限制。

## GitHub 首次提交前核查（2026-11-14）

用户授权：核查当前项目，通过后提交并推送至 `Acorn2/devnook`。本轮没有发布 Release 或安装包。

用户进一步确认只推源码：Pages 取消 `push` 触发，仅保留手动发布；常规 CI 增加官网构建和检查。部署说明同步更新，不启用 Pages、不触发发布工作流，也不修改仓库设置。

- 核对当前分支 `main`、远端地址及首次提交状态；尊重现有应用、图标、官网与文档的本地成果，不改业务实现。
- 匿名化开发记录中一处个人目录，第三方说明中的旧括号图标描述同步为当前 N 标志。发布候选不含依赖目录、构建产物、安装包、临时文件、真实环境配置或数据库。
- 对候选文件进行密钥、私钥、凭证 URL 与个人路径的静态模式扫描。命中项逐一核对为隔离测试用合成凭证或原始需求文档的 `/Users/user` 占位路径，未发现真实密钥；此检查不构成完整安全审计。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 前端 | `pnpm check`、`pnpm format:check`、`pnpm test:ui`（11 项）、`pnpm build` 通过 |
| Rust / 原生 | `cargo fmt --all -- --check`、`pnpm check:rust`、隔离数据目录下的 `pnpm test` 全工作区测试、`pnpm build:cli`、`pnpm tauri build --debug --no-bundle` 全部通过。ignored 项为既有内部子进程 fixture，没有失败 |
| 官网 / README | `pnpm site:build`、`pnpm site:check` 通过，包含 3 项发布状态测试与 66 个本地站点引用；两版 README 的 70 处本地链接 / 图片 / 锚点通过，README 与站点源码 Prettier 检查通过 |
| 依赖审计 | `pnpm audit --prod --json` 返回 0 项已知漏洞。仅覆盖该次注册表查询支持的生产 JavaScript 依赖，不等同于 Rust 依赖或完整供应链审计 |
| 工作流 / 排除项 | 两份 GitHub Actions 工作流通过 `actionlint`；`git check-ignore` 确认 `.tmp/`、`target/`、`node_modules/`、`.env` / `.env.local` 与数据库不进入首次提交 |
| 服务保全 | 只读 `ps` 确认原 Core 65870、runner 95832、服务进程 95836 / 95880 仍在运行。测试只操作临时数据和测试进程，未重打包覆盖正在使用的 `.app`，未停止现有后台或服务 |

暂存内容的空白检查通过，唯独保留原始需求资料中既有的 5 处文本图表行尾空格，遵循不改原始需求文档的项目约束。

本地浏览器布局与交互沿用上一节实际验收记录，本轮没有重新操作界面；Safari / Firefox、Intel / 旧 macOS、线上 GitHub 渲染与远端 CI 尚未在本轮验证。检查记录位于忽略目录 `.tmp/github-preflight-20261114/`。

## 内折角 N 图标原生打包与 Dock 验收（2026-11-14）

用户确认使用本版图标，要求重新打包并打开应用查看真实 Dock 效果。

- 核对运行路径：旧 GUI 使用 `target/latest/debug/bundle/macos/DevNook.app`，Core / runner 使用 `target/debug/bundle/macos/DevNook.app`。Release 路径无进程 / lsof 引用，旧 DMG 未挂载；将旧 Release 产物暂存以供回退，再复用固定 Release 输出重新生成 `.app` / DMG。
- 初次 bundle 在仅配置 1024px 普通 PNG 时复现 `No matching IconType`。按 Tauri 打包器实现补齐由同一 SVG 生成的原生 `icons/icon.icns`，`tauri.conf.json` 同时配置 ICNS / PNG；原生尺寸族覆盖 16–1024px，ICNS 的 1024px RGBA 像素与已确认 PNG 完全一致。DESIGN 补充实际导出与打包约束。
- 完成最终 Release 构建与本机 ad-hoc 签名，生成固定的 `target/release/bundle/macos/DevNook.app` 和 `target/release/bundle/dmg/DevNook_0.1.0_aarch64.dmg`。`record-release.mjs` 更新 SHA256SUMS、实际体积和官网本地候选元数据，仍为 `published: false` / `url: null`；RELEASING 区分初始历史基线和当前产物。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 构建与打包 | 最终 `pnpm tauri build --no-bundle` 通过，包含 TypeScript / Vite production build；`APPLE_SIGNING_IDENTITY=- CI=true pnpm tauri bundle --bundles app,dmg` 通过。最终 `.app` 14,911,803 字节（14.91 MB），DMG 4,695,106 字节（4.70 MB） |
| 包签名与图标 | `codesign --verify --strict --deep`、`plutil -lint` 通过；包中 CFBundleIconFile 指向 `icon.icns`，与源 ICNS 字节一致，读取的 1024px 图像与选定 PNG 像素一致 |
| DMG 与元数据 | `hdiutil verify` 通过；只读挂载后 Info.plist、主程序、ICNS 与固定输出 `.app` 逐字节一致，Applications 链接存在，挂载包签名复验通过，随后正常卸载。DMG SHA-256 与 `site/release.json` / SHA256SUMS 一致 |
| 原生与 Dock | `/usr/bin/open -n` 从完整 Release 路径打开；确认新 GUI PID 58455。只读截图回读：真实窗口侧栏使用新 N、后台已连接，Dock 显示蓝青内折角 N；绿色大括号属于仍在运行的旧 GUI，没有清除缓存或重启 Dock |
| 服务保全 | 开工 / 打开后只读 v10 Core 快照完全一致：原服务配置版本、运行 / 历史、PID / PGID、runner 启动身份、停止标记、DEVNOOK TCP 监听均保持。原 Core 65870、runner 95832、服务进程 95836 / 95880 的启动身份未变，没有服务启停或配置写入 |
| 官网 | 更新发布元数据后 `pnpm site:build`、`pnpm site:check` 通过：3 项发布状态测试、两个语言路由及 36 个本地链接 / 资产通过 |
| 清理 | 新包 / DMG / Dock 通过后，核对字面量路径、包身份与运行引用，清理本轮旧 Release 回退 `.app`、旧 DMG 和旧校验文件；仍被旧 GUI / Core / runner 使用的 debug / latest 包保留至正常退出 |

CUA Dock 接口超时后停止该界面操作，改用 screenshot 技能的只读原生截图完成验收；没有使用 AppleScript / 私有 UI 自动化或更改系统设置。新版窗口保持打开供用户查看，未退出旧 GUI / Core / runner，没有提交、推送或公开发布。证据与只读快照位于忽略目录 `.tmp/icon-dock-20261114/`；原始截图保存在系统临时目录，仅将 Dock 局部用于交付。Intel / 旧 macOS 与 Apple 公证未在本轮验收。

## N 图标内折空间与小尺寸优化（2026-11-14）

用户确认：保留现有蓝青配色，强化 N 的独特性、空间收拢感与小尺寸辨识度。

- 比较曲线内折、额外围合与内折角三个矢量方向，并检查曲线端头缩短后的轮廓。本版采用连续的内折角 N：右上与左下端部向内折回，保留两个开放容纳空间，避免增加细小节点、功能图案或额外装饰。
- 将三段分离笔画合成一条填充路径，沿用主题 `#91baff` → `#80d9ed` 蓝青色，移除单独的浅色斜带及暗色末端；主图形边界从 132×124 增至 152×144（256 单位画布），宽高分别增加约 15% / 16%。底板、透明外边距和消费入口保持。
- 更新桌面 / 官网的同内容 SVG 与 1024px PNG，现有侧栏、关于区、README 与 favicon 引用同步获得新图形。DESIGN 更新实际造型与取舍，`docs/images/devnook-icon-preview.png` 更新为前后对照及浅暗 / 单色尺寸预览。本轮未修改 TS / CSS、依赖或业务代码。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 构建 | `pnpm build` 通过，包含 `tsc --noEmit` 和 Vite production build；`pnpm site:build`、`pnpm site:check` 通过，3 项发布状态测试及两个语言路由 / 36 个本地链接和资产通过 |
| 资产一致性 | SVG XML 解析成功，主图形为单条填充路径，不含 script / style / filter；桌面与官网 SVG 字节一致，两处 production build 资产与源图一致；桌面 PNG 与 Tauri 从该源图生成的 1024px 文件一致 |
| PNG 导出 | Tauri CLI 导出 16 / 24 / 32 / 48 / 64 / 128 / 1024 px，检查各 PNG 头的实际宽高一致；`sips` 确认桌面图标为 1024×1024 且带 Alpha |
| 视觉 | ego-browser 隔离文件页的 15 张图片全部解码成功，1200×860 对照页无横向 / 纵向溢出；回读浅色 SVG、暗色原生尺寸 PNG 与单色 16 / 24 / 32 / 64 px 截图，N 轮廓保留，单色下仍有内折特征；正式对照图已保存 |

未重打包或替换 `.app` / DMG，未复测真实 Dock / WebView；历史安装包图标与 `site/release.json` 保持实际原状。未修改 Core、服务、数据库、协议或用户设置；没有提交、推送或发布。浏览器任务已结束，Ego Lite 提示可更新，本轮未升级。修改前六个文件已备份至忽略目录 `.tmp/icon-refine-20261114/originals/`，探索稿与尺寸导出也位于本任务临时目录。

## 蓝青 N 形应用图标（2026-11-14）

用户要求结合产品 UI 配色与已实现功能重新设计应用图标。

- 使用两个圆角模块与连接带组成 N，表达项目归集、开发服务及项目关联 / 服务依赖编排。底板沿用深石墨界面颜色，模块使用主题已有的蓝色与青色，替换旧绿色大括号图案。
- `apps/desktop/public/devnook.svg` 为矢量源，`site/assets/devnook.svg` 保持字节一致；通过项目已有 Tauri CLI 导出 1024×1024 透明 PNG，更新 `apps/desktop/src-tauri/icons/icon.png`。不增加依赖。
- 侧栏与关于区使用同一 SVG，移除仅服务于旧大括号的旋转、染色、底板与阴影；README / favicon / 官网沿用现有引用获得新图形。设计依据、导出方法写入 DESIGN，预览交付为 `docs/images/devnook-icon-preview.png`。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 桌面前端 | `pnpm build` 通过，包含 `tsc --noEmit` 与 Vite production build；`pnpm format:check` 通过 |
| 官网 | `pnpm site:build`、`pnpm site:check` 通过；3 项发布状态测试通过，两个语言路由及 36 个本地链接 / 资产通过 |
| 资产 | SVG XML 解析成功，桌面 / 官网 SVG 字节一致；PNG 与 Tauri 导出文件字节一致，`sips` 确认为 1024×1024 且有 Alpha；生产构建中的桌面 / 官网 SVG 均与源文件一致 |
| 视觉 | ego-browser 隔离文件预览的 11 张图片均成功解码，无水平溢出；回读浅暗背景与 16 / 32 / 48 / 64 px 的 SVG / PNG 对照截图，N 形可辨认，截图保存为正式预览交付物 |

未重打包或替换现有 `.app` / DMG，未变更 `site/release.json`，已生成的历史安装包仍保留原有图标。未复测原生 Dock / WebView；未修改 Core、服务配置、协议、数据库、发布元数据或用户服务，没有提交、推送或发布。浏览器任务已结束；Ego Lite 提示有更新，本轮未升级。改动前原文件备份位于忽略目录 `.tmp/app-icon-20261114/originals/`。

## 端口冲突交互、端口筛选与 CI（2026-11-14）

用户确认先补齐这三项，IDEA 插件暂缓；保留浅暗主题、安全确认及正在运行的服务。

- 未活动服务详情根据完整 TCP 快照列出相同预期端口的占用者，展示端口 / PID / 命令 / 目录 / 项目 / 已核验来源；合并同一 PID + 端口的地址绑定。查看进程跳转端口页并定位 PID，查看托管服务退出原项目上下文、清除服务页筛选并定位配置，均不执行控制请求。
- 端口已释放、仅有其他活动配置的预期端口、快照读取失败分别说明，不把历史阻断或预期端口当作当前监听。停止继续走既有服务 / 进程详情入口，不增加自动停止、自动重试或接管。
- 端口页“更多筛选”增加运行时、进程类型、启动来源，可与搜索 / 项目 / 地址族组合，支持清除全部筛选和收起后保留条件。运行时只是进程名的展示分组；未知原生程序不按项目技术栈猜测语言，类型 / 来源沿用 Core 事实，不影响归属或控制权限。数量分母保持完整成功快照；失败显示“—”，成功空快照显示 0。
- 新增 GitHub Actions：main 推送 / PR / 手动触发，macOS 15 上执行锁文件安装、TS / 格式 / 界面数据单测、Rust 检查 / 集成测试、CLI / 原生构建。Action 固定提交，令牌只读，不持久化 checkout 凭证，使用隔离应用目录，无发布或部署步骤。
- 新增 4 项界面数据单测，更新 README / DESIGN；Core / IPC / SQLite / 服务控制逻辑、依赖版本及原始需求文档未修改。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 自动回归 | `pnpm test`：73 passed、0 failed、5 ignored（测试内部调用的子进程 fixture）；`pnpm test:ui`：4 passed，覆盖筛选叠加、分母与双栈去重、未知 / 外部与托管区分、冲突展示不改变原数据 |
| 静态与构建 | `pnpm check`、`pnpm format:check`、`pnpm build`、`pnpm check:rust`、Rust 格式、`pnpm build:cli`、`pnpm tauri build --debug --no-bundle` 通过；最后 CSS 间距调整后再次原生构建并重打包 |
| CI | 从官方 release 校验 SHA256 后运行 actionlint 1.7.12，最终工作流检查通过。修正了 job env 不能使用 runner context 的问题，改为通过 GITHUB_ENV 写入隔离目录；未提交 / 推送，因此尚无 GitHub 远端运行结果 |
| 隔离界面 | ego-browser + 实际 React + 内存适配器：多维组合、1 / 4、0 / 4、清除后 4 / 4、快照失败“—”、成功空快照 0 / 0、未知归属只读、停止确认空 PID 禁用 / 取消通过。两种冲突跳转无启停 / 保存 / 归属修改请求；占用消失、配置预留与快照失败文案通过；无捕获的页面错误 |
| 外观与窗口 | 回读浅色与暗色截图；1380×900 冲突详情保留日志主体，两主题 760×560 的高级筛选无横向溢出、端口工作区高度约 254px；长冲突内容及右侧详情独立滚动 |
| 打包与原生 | 首次在空闲 latest 输出打包并打开；最终补齐“未关联项目”检索兼容和跨项目上下文后，用空闲固定 `target/current` 重打包，约 44.77 MiB，未覆盖已打开的 latest。最终 ad-hoc 签名、codesign strict/deep、plutil 校验通过并由 open -n 打开；原生 AX / 截图确认真实端口 56 / 56、更新时间、三个新增筛选和后台已连接；搜索“未关联项目”为 41 / 56，清空恢复 56 / 56，窗口留在端口页 |
| 服务保全与清理 | 比较任务前后只读快照，原 GUI / Core / runner PID 和启动身份保持，原 backend 的 run ID / PID / PGID / READY 状态未变。只关闭本轮启动的 Web UI，浏览器任务已结束；清理无运行引用的旧 current / pending-external-handoff .app 与两轮打包输入副本。原 standard / preview 和中间 latest GUI 仍运行，按规则保留；闲置 localhost 验收 Core 入口仅更新到 current 包路径，未启动 |

验收数据和日志位于 `.tmp/ports-ci-20261114/`，内存适配器不作为本机业务事实；未向正式库写入测试项目 / 配置、确认归属或向用户服务发送信号。GitHub 远端 CI、Intel / 旧 macOS、本轮原生界面中的冲突控制流程未实测；安全控制已有 Rust 回归覆盖。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 外部服务正常停止与显式切换为托管（2026-11-13）

用户确认：外部服务详情提供“停止外部服务”和“配置为 DevNook 服务”；保存后单独确认是否停止旧实例并由 DevNook 启动，保留身份与 PID 确认，不自动强杀。

- [x] `external_service.rs` 在 Core 重新核验外部服务入口、已确认项目与目录，提供当前目录的清单建议；无法唯一识别时留空。新建配置与既有配置独立，不复放观察命令、不读取旧环境或迁移 IDE 调试会话。
- [x] 详情正常停止复用 30 秒单 PID 确认与 `confirm_stop`，仅 SIGTERM；配置先通过既有编辑器保存，再呈现“仅保存配置 / 输入旧 PID 确认切换”。关闭或取消不发控制请求，执行期间禁止重复确认与关闭。
- [x] 60 秒一次性切换绑定原指纹 / 归属 / 监听，新配置版本 / 最近运行及完整依赖版本 / 运行实例。确认使用现有服务操作锁，先预检可执行文件、Keychain 引用、依赖 READY 和其他端口占用，再正常停止旧 PID，在配置宽限内等待退出和原 / 新预期端口释放，复用 start_run / runner。
- [x] 旧身份变化、配置或实例变化、占用、忽略 SIGTERM、仅关闭端口但进程继续运行均阻断；原启动器另起的监听保留，不停止外部进程组、不自动强杀或恢复旧实例。macOS 僵尸进程的正向 ps Z 状态可确认为执行结束，身份暂不可读且 PID 仍存在时继续等待。
- [x] IPC / Tauri 保持薄适配；新增本地接口，MCP 即使持有效范围授权也不能调用。确认只存内存、Core 重启失效，不恢复外部切换；SQLite v3 / UDS v10 不变，无新依赖或迁移。保留工作区已有的高级接入及端口页布局改动。
- [x] 更新 AGENTS / MVP / ARCHITECTURE / TECH-STACK / DESIGN / README 和专项清单；没有修改原始需求文档。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 完整自动回归 | 最终 `pnpm test`：73 passed、0 failed、5 ignored（仍为测试内部调用的 fixture）。新增 6 项真实 macOS 进程 / 端口切换集成测试、1 项过期令牌单测、1 项 MCP 权限集成测试 |
| 安全与失败路径 | 验证保存 / 预览不执行、不覆盖旧配置；PID 错误与重复确认、过期、旧指纹、新配置 / 依赖 / 运行实例变化、其他端口占用、可执行文件不存在、依赖未就绪均阻断。真实旧进程忽略 TERM 或仅关闭监听仍不启动新实例；被父启动器重建的监听继续存活，未整组停止 |
| 静态与构建 | `pnpm check`、`pnpm format:check`、`cargo check --workspace`（经 rust-local）、`cargo fmt --all -- --check`、`pnpm build:cli` 通过；`pnpm tauri build --debug --no-bundle` 包含 TypeScript / Vite production build 与原生编译，通过 |
| 隔离界面闭环 | ego-browser + 实际 React + 隔离 v10 Core：取消停止不发送信号；正确 PID 停止后外部条目消失；“仅保存配置”后旧 PID / OBSERVED 监听仍在、配置无运行记录；确认切换后旧 PID 退出、新监听具有 DEVNOOK + service_id、READY 且日志出现新输出。错误 PID 时按钮禁用，页面无捕获错误 |
| 外观与窗口 | 浅色完整切换预览、暗色外部详情与停止确认截图已回读；760×560 下页面 / 外部详情 / 停止弹窗无横向溢出，确认与取消按钮在视口中可见 |
| 新包与包内复测 | 固定 `target/pending-external-handoff/debug/bundle/macos/DevNook.app`，约 44.77 MiB。重建 ad-hoc 资源签名后 `codesign --verify --strict --deep` 通过。包内 Core 读回“仅保存”配置及已停止运行，再用额外隔离 fixture 完成建议 → 保存 → PID 切换 → 新 runner READY → DEVNOOK 端口 → 新日志 → 正常停止，全部通过 |
| 收尾与保全 | 证据及隔离数据位于 `.tmp/external-handoff-20261113/`。测试托管服务正常停止，三个外部 fixture 正常退出，浏览器任务已结束；Web UI / HTTP 适配器 stopped，独立与包内 Core 均经 shutdown 退出码 0。仅清理本轮已核验的打包输入硬链接，保留新待切换包；既有 standard / current / latest / preview 包仍有运行引用，未替换、停止或删除 |

未人工复测原生 WebView 的点击流程、Intel / 旧 macOS、真实 Keychain 交互、IDEA Debug 或用户真实服务；没有写入正式数据库 / MCP 客户端配置或创建真实授权，没有提交、推送或发布。新包需在用户按正常流程停止旧版托管服务并退出旧应用 / 客户端、等待旧 Core 退出后使用。Ego Lite 提示可更新，本轮未升级。

## CLI / MCP 收入设置高级选项（2026-11-13）

用户批准：保留 CLI，将 MCP 接入与授权管理移到设置的高级选项，移除单独侧栏入口，并改写用途说明。

- [x] 移除 `agents` 顶级页面和侧栏“CLI 与 MCP”。“设置 → 高级选项”默认收起，先呈现 CLI 用途、路径提示和服务 / 日志 / 启动方案命令示例。
- [x] “AI 助手接入（MCP，可选）”单独折叠，按复用配置与桌面管理的具体用途、手动连接客户端、按需创建控制授权的顺序说明。空状态只说明尚未创建授权，不再声称已处于只读连接；数量仅统计未撤销记录，不推断配置仍有效。
- [x] MCP 展开后才挂载授权与审计组件，收起时停止轮询。读取中、后台未连接和读取失败分别展示，首次读取失败不显示空授权状态。授权组件位于普通设置表单之外，保留既有创建、令牌展示与撤销流程，未修改 Core 权限边界。
- [x] 更新 README / DESIGN / Phase 3 的当前入口和说明，修正设置“关于”中 CLI / MCP 尚未实现及旧 Phase 标签。未修改原始需求、CLI / MCP 协议、Rust 业务代码、数据库或依赖。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 静态与构建 | `pnpm check`、`pnpm format:check` 通过；`pnpm tauri build --debug --no-bundle` 内含 TypeScript / Vite production build，编译通过，输出 `target/debug/devnook-desktop` |
| CLI 保留 | `./target/debug/devnook --help` 正常退出，project / service / workspace / ports / agent / mcp / status 命令均保留；未调用真实库的控制命令 |
| 导航与折叠 | ego-browser 检查侧栏只剩项目、收藏、服务、端口与设置；高级默认关闭，打开后 CLI 可见、MCP 仍关闭。两者尚未展开时 agent_grants / access_audits 请求数为 0；展开 MCP 后读取真实隔离 Core，收起后跨过 2.5 秒周期没有新增授权查询 |
| 表单与键盘 | 从高级选项打开既有授权编辑器并取消，DOM 无嵌套 form，授权区域不属于设置 form，未发出 save_settings / create_agent_grant / revoke_agent_grant。MCP 与高级选项可用 Enter 展开 / 收起 |
| 外观与错误 | 回读暗色 1380×900 和浅色 1024×768 截图；760×560 最小窗口文档及接入区域无横向溢出。模拟链路中断采用停止本轮 HTTP 适配器，实际显示读取失败、禁用创建且不展示空授权结果；正常路径无捕获的页面错误 |
| 隔离与收尾 | 测试数据位于 `.tmp/advanced-integrations-20261113/`，浏览器通过只读 HTTP / UDS 适配器读取真实 v10 Core，无样例业务数据。浏览器任务已结束，Core 经 shutdown 退出码 0，Web UI / HTTP 适配器已停止；复用已停止的 localhost QA 定义，未停止用户运行 |

未复测原生 WebView 交互、真实客户端连接、令牌生成和撤销。没有创建真实授权、修改用户 MCP 配置或写入正式数据库；没有提交、推送或发布。本轮仅编译未生成 / 替换 `.app`，现有运行中的应用、Core 与 runner 保持。Ego Lite 提示可更新，本轮未升级。

## 项目服务与开发工具自身监听分离（2026-11-13）

用户批准：项目服务页保留真正的外部项目服务，Codex / WorkBuddy / IDE 自身的端口及用途不明的监听仅在端口管理中显示，不新增工具分组，不自动接管。

- [x] Core 按每个监听 PID 的内核可执行路径、脱敏命令实际入口及已导入项目路径识别 SERVICE / DEVELOPMENT_TOOL / UNKNOWN；不遍历工程文件、不执行项目命令。仅 cwd 匹配或人工项目确认不构成服务证据。
- [x] 保留 Node / Bun / Deno 项目入口、常见 Web / Expo 开发服务器、Python 服务入口、Java JAR / 项目 classpath、.NET DLL、PHP / Ruby 服务及本地编译产物等静态线索。开发工具的子进程独立识别；IDE 内置 JVM / Node 启动真实项目入口时仍显示。无法解析的自定义入口保守留在端口管理。
- [x] 服务页改为“外部启动的服务”与“关联服务…”，关联列表仅展示服务入口；项目卡片、统计和端口状态筛选使用同一服务分类。完整 TCP 快照与端口管理不删减；手动确认不改变分类，source / service_id / can_stop 及控制安全逻辑不变。
- [x] UDS v10 拒绝缺少入口分类的旧 Core；SQLite 保持 v3，无迁移或新依赖。README / ARCHITECTURE / AGENTS / Phase 3 当前协议说明已同步。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 自动化回归 | 最终 `pnpm test`：65 passed、0 failed；5 ignored 仍仅为业务测试启动的 fixture。新增 3 项分类单测和 1 项真实 macOS 集成测试；既有人工归属与托管端口测试补充分类断言 |
| 分类反例 | 工具提示词中的 server.js、后续参数中的项目路径、Node eval / Python -c、路径越界、Java 属性中的 Spring 字样、缺少可执行路径及旧 JSON 保持未知；工具入口、语言服务与 Gradle / IDE JVM 不成为项目服务。覆盖 JBR 路径含空格、Java 独立参数、Next / Gunicorn 进程标题、Phoenix、其他运行时服务入口 |
| 真实 TCP / 导入 | 一次性 Node 工具 fixture 自己监听，并启动真正的 server.js 子进程；导入前子进程 UNKNOWN，导入后立即 SERVICE，工具一直 DEVELOPMENT_TOOL；全部绑定仍存在且为 OBSERVED，无 service_id、未创建服务。手动确认工具项目归属不会改变工具分类，也不停止或接管任一进程 |
| 静态与构建 | `pnpm check`、`pnpm format:check`、`pnpm check:rust`、`cargo fmt --all -- --check`、`pnpm build:cli` 通过；`pnpm tauri build --debug --no-bundle` 包含 TypeScript / Vite production build，通过 |
| 隔离界面 | ego-browser 通过只读 IPC 适配器连接真实 v10 Core；真实工具 / 子服务 / Node eval 三个监听均关联同一测试项目，卡片只统计 1 个服务，服务页只列子服务并显示实际端口，关联弹窗排除工具与未知用途。端口管理实测三类 PID 均可搜索，工具详情可读取；浅 / 暗服务页截图均已回读。状态筛选下拉新文案可见，最终切换断言未完成，不将其记为已通过的人工交互 |
| 协议 | 隔离 v10 Core 拒绝 v9 请求后仍正常响应 v10；既有 deferred GUI / CLI 握手回归通过 |
| 包与原生 | 新包约 44.07 MiB，ad-hoc 签名与 `codesign --verify --strict --deep` 通过。沿用用户此前正常切换新版的明确授权，确认真实 backend 为 STOPPED 后正常退出旧 GUI / 空闲 Core；未再停止任何用户服务、未强杀。核对无进程使用旧包后替换标准输出，删除旧包、打包输入硬链接及空 staging 目录 |
| 正式库核对 | 新包原生窗口与实际 Core v10 已打开；读回对比 2 项目完整档案、设置、默认应用 / 收藏和服务配置均保持，backend 仍 STOPPED。实际项目下的 Codex 监听仍返回 DEVELOPMENT_TOOL / OBSERVED；原生项目卡片显示未检测到服务监听，项目服务页“外部启动的服务”为 0，无 Codex 或额外工具分组 |
| 收尾 | 只保留 `target/debug/bundle/macos/DevNook.app`，无其他 DevNook 打包副本。QA 浏览器任务关闭，隔离 Core 正常 shutdown，Web UI / HTTP 适配器 stopped，3 个临时 TCP 进程均已退出；正式新版窗口 / Core 保留给用户 |

验收材料位于忽略目录 `.tmp/external-services-20261113/`；只有 fixture 导入隔离库，没有写入正式测试项目、授权或 MCP 客户端配置。入口分类为静态展示规则，自定义启动器和无法读取 / 解析的命令可能只在端口管理显示；未做旧版 macOS / Intel / VoiceOver 验收。Ego Lite 提示有更新，本轮未升级。未提交、推送或发布。

## 旧 Core 不兼容导致原生窗口启动崩溃（2026-11-13）

用户反馈新版 `.app` 打开失败。只读请求实际旧 Core 的 v9 ping 返回协议不一致；随后用私有目录的同响应 UDS fixture 启动原始新包，真实桌面进程以 SIGABRT 退出，日志明确为 `Failed to setup app ... Core 协议版本不一致`。根因是 Tauri setup 调用 eager CoreClient::new，在创建窗口前将正常的连接错误变为 setup failure；前轮浏览器 / 包内 Core 验收漏掉了该原生失败路径。

- [x] Core 新增目录校验仍严格的 new_deferred，GUI setup 使用该入口；CLI / with_launcher 保持 eager 握手。每次请求的身份、权限、协议校验及写入不重试保持原逻辑。
- [x] 前端单独展示连接状态、实际错误与“重新连接”，重连期间禁用重复点击；不兼容时说明正常退出旧服务 / 客户端的方法。不可用项目数显示 `—`，空态明确为项目库暂不可用，不误导为空库。主题仍可独立切换。
- [x] 新增 2 项传输回归：无后台时构造不连接 / 不 spawn；旧 Core 拒绝后同一 GUI 客户端可重试恢复，CLI 仍拒绝旧协议；危险目录 / socket 不被访问。专项通过；`pnpm test` 全量 61 passed、0 failed，5 ignored 仍为被业务测试调用的 fixture。
- [x] `pnpm check`、`pnpm format:check`、`cargo fmt --all -- --check` 通过。
- [x] 新包构建 / 签名；同一不兼容 fixture 下真实 macOS 原生窗口正常打开，显示协议错误、升级说明与重新连接，依赖 Core 的操作禁用，主题与原生标题栏切换仍生效，没有 setup panic。
- [x] 保持同一原生窗口存活，停止隔离旧协议 fixture，再启动同目录真实 v9 Core。实际点击“重新连接”后提示消失、显示 Core 已连接，扫描入口恢复，读取真实 TCP 快照；没有重启 GUI。原生验收窗口经 Cmd+Q 正常退出，私有 Core 经 shutdown 退出。
- [x] 用户明确授权“正常停止 backend 并切换新版”后，核对旧 Core v7 / PID 与指定服务 ID，调用其正常 stop_service；轮询确认 backend 为 STOPPED、配置 revision 不变，再通过 shutdown 正常退出旧 Core。没有强杀、没有控制其他服务。
- [x] 新版窗口正常退出后，核对两个包的 Bundle ID / 签名与使用状态，删除标准路径旧包，将修复包移动到 `target/debug/bundle/macos/DevNook.app`，清理仅含本轮打包元数据的空 staging 目录。新包约 43.95 MiB，签名复验通过。
- [x] 从标准路径重新打开真实原生应用，窗口显示 Core 已连接、原有 2 项目 / 1 收藏，使用真实用户数据目录的 v9 Core。只读逐项对比项目 ID / 名称 / 默认应用 / 分类，以及 backend ID / 名称 / 配置 revision 均保留；收藏项目与用户此前截图一致。backend 保持 STOPPED，未自动启动。

验收位于 `.tmp/startup-recovery-20261113/`。授权前，真实旧 Core 仅查询只读 ping / list_services；授权后，仅正常停止指定 backend 并 shutdown 旧后台，保留项目与服务配置。UDS 继续 v9、SQLite 继续 v3，无 schema 迁移、无依赖变更、未提交或推送。原生失败复现与修复日志由 localhost 保留；只有标准路径最新 `.app`，没有旧包或额外验收副本。隔离验收服务均已退出，真实新版窗口与 Core 留给用户使用。

## 浅色 / 暗色与分层配色（2026-11-13）

用户要求改善当前整体灰绿色配色，支持浅色 / 暗色，具有科技感、分层和渐变。本轮以冷灰浅色、深石墨暗色、蓝青操作渐变实现，共用语义颜色；保留现有信息架构和服务安全流程。

- [x] `Appearance.tsx` 提供顶栏快捷切换和设置双主题预览，立即生效并本机记住；不依赖 Core、设置保存或业务配置。
- [x] 首屏同步 `public/appearance.js` 在绘制前读取显式 light / dark，缺失 / 非法值使用系统初始配色；切换时避免新旧颜色交叉过渡，存储写入失败仍生效并提示未保存。
- [x] 原有 CSS 颜色迁为语义变量，`theme.css` 覆盖背景 / 侧栏 / 卡片 / 输入 / 浮层 / 日志及成功、警告、危险、信息色，OKLCH 配色保留旧 WebView 的十六进制兼容值。无新增依赖。
- [x] 主窗口仅新增 `core:window:allow-set-theme`，通过现有 Tauri API 同步应用标题栏；不改变系统主题、Core 权限、SQLite 或 UDS v9。原生标题栏实际交互未人工验证。
- [x] 根据 README / MVP / 现有组件和用户方向补充 `PRODUCT.md` / `DESIGN.md`，同步长期规则及使用说明。
- [x] 更新同一个待替换 `.app`、签名、包内 Core 与验收进程收尾；上一轮待替换包已被本轮覆盖，没有新增 `.app` 副本。
- [x] 标准路径替换与旧包清理：后续原生启动修复中，经用户明确授权正常停止 backend / 退出旧 Core 后完成，详见本文件首节。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 检查与构建 | `pnpm check`、`pnpm format:check`、`pnpm build`、`pnpm tauri build --debug --no-bundle` 通过。同步启动脚本实际复制到 production dist，源码无旧颜色字面量；主题与组件配色位于统一变量体系 |
| Rust 回归 | `pnpm test` 全量 59 passed、0 failed；5 个 ignored 仍仅为业务测试调用的 fixture。Tauri 能力文件和桌面构建通过，未扩大进程 / 服务权限 |
| 主题行为 | 真实 React + 隔离 Core v9：顶栏切换、设置选择、Space 键操作、重载保留、DOM 首次就绪时已应用主题、非法偏好按浅 / 暗系统配色回退、显式暗色优先于系统浅色通过。浏览器预览无 Core 时仍可切换，业务保存保持禁用 |
| 错误路径 | 仅在测试页面注入当前主题键的 Storage 写入拒绝：本次主题仍改变、设置显示无法保存；恢复存储后再次选择即保存并清除提示。没有清空其他站点 / origin 数据；收尾仅移除本轮主题测试键，恢复原先无偏好的状态 |
| 颜色 / 层次 | 已回读两主题项目列表、设置、应用菜单、筛选下拉、服务建议、真实端口列表和浅色空扫描弹窗截图。日志与四类状态以明确标注的临时样式探针检查，未制造真实运行状态；探针已移除 |
| 对比度 | 浏览器 canvas 按实际 computed color 转为 sRGB，抽查项目标题 / 路径 / 标签 / 导航 / 表单等及状态、主按钮端点，最低浅色 4.79:1、暗色 4.52:1，无低于 4.5:1 的抽查项；不是全界面无障碍认证 |
| 窗口尺寸 | 760×560、1024×768、1380×900 项目页无横向溢出，顶栏切换入口在视口内；760×560 设置选项可正常滚动到视口并切换，两个选项完整显示。保留原有响应式断点和 reduced-motion 行为 |
| 业务隔离 | 测试库始终为前轮 9 个 fixture 项目、0 服务；本轮未导入、保存服务、启动工程命令或创建授权。仅从 macOS 读取真实 TCP 快照；生产库、Keychain、MCP 客户端配置与用户运行未操作 |
| 打包 / 签名 | 复用已构建桌面二进制，在同一 `target/pending-service-eligibility` 输出执行 `pnpm tauri bundle --debug --bundles app`，成功生成约 43.99 MiB 新包；SHA-256 与旧待替换包不同。ad-hoc 签名及 `codesign --verify --strict --deep` 通过；临时 bundle 输入硬链接已删除 |
| 包内 Core | 新包内 Core 启动同一隔离库，9 项目 / 0 服务、默认扫描 9 候选 / 26 目录 / 51 条目及 Electron 的 `npm run web` 建议保持；v8 拒绝后 v9 继续正常响应。随后 shutdown 正常退出并再次验证签名 |
| 产物收尾 | 盘点 target / .tmp，仅本轮最新待替换包及标准路径仍运行的旧包，无其他旧 dmg / pkg / app.zip / 验收副本。用户 PID 45262（Core）、45630（runner）、83564（桌面）仍引用标准包，未终止；所有本轮 localhost 验收服务均 stopped / exited |

预览图： [浅色](images/appearance-light.png)、[暗色](images/appearance-dark.png)，均为隔离测试项目。详细验收材料在 `.tmp/appearance-20261113/`。浏览器任务已结束，HTTP 适配器与 Web UI 已停止，隔离 Core 已正常 shutdown。Ego Lite 提示更新，未升级；未提交、推送或发布。剩余未测：原生 macOS 标题栏人工交互、旧 macOS WebView / Intel 真机、VoiceOver。

## 本机服务资格与原生客户端过滤（2026-11-13）

用户授权：默认排除纯 iOS / Xcode 客户端，补齐同类移动、桌面 GUI 和游戏工程；保留 Swift 服务端、混合仓库后端及 Expo / React Native 开发服务器，补测试、验证、打包并清理旧包。范围先记录在 [service-eligibility-plan.md](service-eligibility-plan.md)。

- [x] Core 统一扫描和启动建议资格，覆盖 Xcode / Swift 移动平台、Android、Flutter、JavaFX / Compose、.NET WPF / WinForms / MAUI / WinUI、Electron / Tauri、Rust GUI、Go Fyne / Gio / Ebiten、Qt / SDL、Python GUI、Unity / Unreal / Godot 的高置信度静态线索。
- [x] 过滤客户端组件及无服务成员的聚合根；保留服务、相关库和独立嵌套 Git 仓库，真实 Git / workspace 边界优先。默认与深度补扫共用资格；不按目录名猜后端，不要求端口，不运行工程命令。
- [x] Node 启动建议保留 Expo / React Native start、独立 Web 服务脚本，跳过原生启动 / 模拟器命令；与扫描使用相同脚本选择。既有档案、偏好、服务配置和运行不做迁移、删除、覆盖或停止。
- [x] 扫描 UI 说明同步；UDS v9 门禁拒绝旧规则，SQLite 保持 v3，无新增依赖。
- [x] 新 `.app` 已打包、签名并验证包内 Core；当前标准输出仍被用户旧桌面 / Core / runner 使用，因此新版暂存固定待替换目录，尚未覆盖运行包。
- [x] 后续原生启动修复中，用户明确授权正常停止 backend 并切换新版；待旧进程正常退出后将最新修复包移到标准输出并清理旧包，详见本文件首节。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 全量 59 passed、0 failed；5 个 ignored 仍为业务测试调用的 fixture。新增 8 项资格测试覆盖纯客户端矩阵、两种模式、同目录 / 同语言多模块后端、Swift 服务端、Expo / RN / Flutter web-server、脚本别名与原生命令过滤、普通 CLI / worker / Qt Network、可选 GUI 依赖、独立嵌套仓库、旧配置与偏好保留、symlink / 注释反例 |
| 静态检查 | `pnpm check`、`pnpm format:check`、`pnpm check:rust`、`cargo fmt --all -- --check` 通过 |
| 应用与 CLI 构建 | `pnpm build:cli`、`pnpm tauri build --debug --no-bundle` 通过；后者实际执行 TypeScript / Vite production build，生成新版桌面可执行文件 |
| 隔离 Core / GUI | 6 类纯客户端根 + 9 个应保留服务项目，默认与 8 层补扫都只返回 9 项目 / 9 服务候选，无 incomplete。默认 26 目录 / 51 条目；补扫 28 目录 / 53 条目。React 通过临时适配器连接真实 Core v9，预览、搜索混合仓库、开启补扫、选择 8 层、重扫、显式导入 9 项目通过 |
| 启动建议 / 无执行 | UI 展示 Electron 工程独立 `npm run web`，没有建议 electron；Core 读取 Expo / RN 均为 `npm run start`。测试库为 9 项目、0 服务；未保存或启动工程命令 |
| 布局 | 已回读 1380×900 混合仓库预览、760×560 补扫和项目服务建议截图；操作栏在视口内、结果独立滚动，无横向溢出 |
| 桌面可执行文件 | 用新构建桌面二进制的内部 Core 模式重启同一隔离库，持久化 9 项目 / 0 服务、默认 / 补扫结果、三个 Node 启动建议再次通过。v8 请求明确拒绝后 v9 仍正常响应，随后 shutdown 正常退出 |
| 打包 / 签名 / 包内 Core | `CARGO_TARGET_DIR=target/pending-service-eligibility pnpm tauri bundle --debug --bundles app` 成功，复用已构建桌面二进制，不复制构建缓存；新包约 43.97 MiB。ad-hoc 签名及 `codesign --verify --strict --deep` 通过。包内 Core 再次确认 9 项目 / 0 服务、两种扫描模式、Node 建议和 v8 拒绝，正常 shutdown 后签名复验通过 |
| 旧包清理状态 | 盘点仅标准旧包及本轮待替换新包，无其他旧 dmg / pkg / app.zip 或验收 .app 副本。标准包仍被 PID 45262（真实 Core）、45630（runner）、83564（桌面）使用；遵守 AGENTS.md，未覆盖 / 删除运行包，等待用户正常退出。新包位于 `target/pending-service-eligibility/debug/bundle/macos/DevNook.app`，替换完成前不能将标准路径称为新版 |

验收位于 `.tmp/service-purpose-20261113/`；仅使用测试清单和私有数据库，不写真实项目库、Keychain、MCP 客户端或系统配置。浏览器任务结束，HTTP 适配器 / Web UI 已停止，隔离 Core 已 shutdown。用户既有旧桌面 / Core / runner 未操作。原生 WebView、Intel / macOS 12、VoiceOver 与自定义 headless / 动态启动器未人工运行验证；静态识别局限已说明。Ego Lite 提示更新，未升级。未提交、推送或发布。

## 项目特征扫描与深度补扫（2026-11-13）

用户要求先规划本轮项目种类，再实现按特征 / 模块声明扫描、前后端与独立嵌套识别，保留补扫和安全上限，验证、打包并清理旧包。设计先记录在 [scanner-features-plan.md](scanner-features-plan.md)，随后实施。

- [x] 规划并实现 11 类工程发现：JS/TS、Java/Kotlin、Python、Rust、Go、.NET、PHP、Ruby、Dart/Flutter、Swift/Xcode、C/C++。沿用现有服务执行机制，不承诺所有框架均能生成启动命令；库与服务候选继续区分。
- [x] Core 增加静态清单解析与共享预算 walker。默认 depth=0 在识别项目后只继续声明模块、常见组件容器和两层浅探，普通源码 / 文档 / 素材 / 隐藏目录剪枝；明确声明及已导入组件定点复查。depth=1–12 保留原递归补扫。
- [x] 声明覆盖 npm / Yarn workspaces、pnpm packages / 排除、Cargo members / exclude / path 依赖、uv workspace、Maven modules、Gradle 字面量 include / includeBuild / file 字面量 projectDir 映射、go.work use、.NET solution / ProjectReference、Dart workspace、CMake / Meson 字面量子目录、.gitmodules path。动态配置不执行，提示补扫。
- [x] 模块展开与普通遍历共用目录枚举缓存、100000 条目、120 秒、取消和深度预算，另有 100000 展开状态、单目录 4096 模块 / 排除声明保护；逐段验证路径、拒绝 symlink / .env / 依赖缓存和所选根外路径。原有 5000 原始候选上限不变。
- [x] GUI 展示默认 / 补扫模式、11 类类型标签、目录及枚举条目两个计数；CLI / MCP / 设置文案同步。Core UDS 升到 v8，避免新版静默使用旧扫描器；SQLite 保持 v3，不自动终止旧 Core。
- [x] 文档、测试、真实目录只读比较、隔离 GUI 验收、应用 / CLI 构建、签名、旧产物核对与验收服务收尾。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 最终全量 51 passed、0 failed；5 个 ignored 仍仅为业务测试调用的 fixture。扫描专项 13 项，新增 6 项覆盖 11 类、声明矩阵、源码剪枝 / 深度补扫、跨生态多模块、独立嵌套与既有组件保留、越界 / symlink / 动态配置；原有进程 / 服务 / 编排 / CLI / MCP 回归通过 |
| 静态检查 | `pnpm check`、`pnpm format:check`、`pnpm check:rust`、本地工具链 `cargo fmt --all -- --check` 通过；最终打包还执行 TypeScript / Vite production build。Core 直接复用锁文件已有 toml 1.2 / quick-xml 0.42 / glob 0.3，无全局依赖或数据库迁移 |
| 剪枝样例 | 临时根内 src / assets / docs 共 300 个子目录、1200 个普通文件及一个非标准服务；默认检查不超过 3 个目录、枚举少于 20 条，未进入这些子树；8 层补扫检查超过 300 个目录、枚举超过 1500 条并找回服务，无命令执行 |
| 真实目录对比 | 新旧 Core 均使用独立空库，对 本机多项目代码目录 只读扫描、未导入真实项目。旧版 12366 目录、命中 100000 条目、154 候选，约 1.387 秒；新版 3178 目录、26230 条目、109 候选，约 0.792 秒。枚举减少 73.77%，目录减少 74.30%；耗时仅为本机单次观察，不作为固定性能承诺 |
| 覆盖差异与限制 | 新版相对旧版少 47 个候选，核对全部位于隐藏插件 / 生成 / 运行目录，符合默认隐藏目录剪枝策略；这些仍可选择具体目录或补扫发现。非隐藏的旧候选均保留，并新增识别两个工程。3 处动态 CMake 子模块仍提示无法静态展开，标记结果可能不完整；未再触发条目上限，不将其写成“保证无遗漏” |
| GUI 默认与补扫 | 真实 React 通过临时适配器连接隔离 Core v8。11 类 + 前后端 / 独立嵌套样例，默认 13 项目 / 7 服务候选，21 目录 / 42 条目；6 层补扫同为 13 项目但 8 服务候选，178 目录 / 348 条目，新增 src/custom/deep。独立嵌套仓库单列且默认不随父项目全选 |
| 导入与持久化 | UI 显式仅选择测试 fullstack 与 independent 两项目导入，库中服务数始终为 0，未生成执行标记。重新打开扫描弹窗使用默认模式，旧的 src/custom/deep 仍定点查到，22 目录 / 43 条目，项目显示已导入可更新；原有偏好保留由回归覆盖 |
| 布局 / 交互 | 实测默认、开启补扫、层数下拉、重新扫描、关键词自动展开、清空 / 勾选 / 预览导入及取消。760×560 / 1380×900 对话框无横向溢出，操作栏在视口内，结果区独立滚动；默认与展开高级设置截图已回读检查 |
| 协议 / 包内 Core | v8 Core 明确拒绝 v7 请求，拒绝后仍正常响应 v8；未自动关闭旧 Core。新包内可执行文件重启隔离 Core，读出 2 个测试项目、0 服务，扫描仍为 13 候选 / 22 目录 / 43 条目，既有非标准组件与独立 Git 边界保留，随后正常 shutdown |
| 打包 / 签名 / 清理 | `pnpm tauri build --debug --bundles app`、`pnpm build:cli` 成功；固定输出 `target/debug/bundle/macos/DevNook.app`，约 43.63 MiB。ad-hoc 签名及 `codesign --verify --strict --deep` 通过。固定路径替换旧包，未创建验收 .app 副本；盘点 target / .tmp 后仅最新 DevNook.app，无旧 dmg / pkg / app.zip，无需额外删除运行包 |

范围内业务主要在 Core scanner / manifests / walk，前端只展示和调用。验收资料在 `.tmp/scanner-features-20261113/`；用户真实项目仅只读扫描，未写真实项目库、Keychain、MCP 配置或系统设置。浏览器任务结束，临时 HTTP 适配器与 Web UI 已停止，隔离 Core 均经 shutdown 正常退出；用户既有桌面 / Core / 服务未被终止。Ego Lite 提示可更新，未升级。原生 WebView、Intel / macOS 12 与 VoiceOver 未人工交互复测。未提交、推送或发布。

## 服务实际端口与外部进程归属（2026-11-13）

用户确认：服务列表 / 详情显示实际端口；项目服务页汇总外部启动的监听进程，导入立即刷新，无法确定时手动确认，不自动接管；验证、打包并清理旧包。

- [x] 服务列表直接显示实际 TCP 端口，详情将地址 / 地址族 / PID 前置，预期端口仍保留在折叠运行详情中。仅使用 Core 已核验的 DEVNOOK + service_id；PROCESS 就绪和预期端口不参与监听推断。搜索包含实际端口与 PID。
- [x] 项目服务页新增独立的外部进程区域，按 Core 归属过滤、按 PID 合并多端口。详情只读展示命令、工作目录和监听地址，不猜测具体由 Codex / WorkBuddy / 终端中的哪个工具启动，不提供服务启停按钮。
- [x] “关联其他进程…”支持按端口 / PID / 命令 / cwd 检索，inspect 核验后显式确认项目。沿用 Core 指纹关联与身份复核；托管进程不能被手动重分配，确认不创建服务、不接管外部进程。
- [x] 导入使旧快照失效并立即补刷；已有查询时排队再取，丢弃过时成功结果。服务运行 ID / 状态变化触发端口补刷。读取失败显示未知并移除实际端口卡片，不影响独立的服务状态。
- [x] 760px 桌面最小宽度改用左右独立滚动区域，列表端口前置；较窄浏览器宽度使用面板内滚动。无新依赖、Core 业务改动、协议或数据库迁移；UDS v7 / SQLite v3 保持不变。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 45 passed、0 failed；5 个 ignored 仍为被测试调用的隔离 fixture。新增 PROCESS + 无预期端口的真实监听回归；同项目的外部监听仍为 OBSERVED、无 service_id，手动确认不接管，停止托管服务不影响外部进程。增强导入前已监听、导入后自动归属、托管进程拒绝重分配断言 |
| 检查 / 构建 | `pnpm check`、`pnpm build`、`pnpm format:check`、`pnpm check:rust`、`cargo fmt --all -- --check`（经本地工具链脚本）通过 |
| 导入与刷新竞争 | React 通过临时 HTTP 适配器连接全新隔离真实 Core。实际 TCP 1421 进程在导入前启动、初始无归属；UI 扫描并确认导入 alpha / beta。特意挂起导入前快照响应，导入后释放：过时查询后立即补取，补取约在导入完成 0.554 秒返回，项目显示既有进程，无需重启进程 |
| 手动关联 | UI 搜索真实测试 Vite 的 1420 端口，显示未关联目录与 PID；显式选择 beta 并确认后立即进入 beta 的外部列表。Core 返回 MANUAL / OBSERVED / service_id=null；新 .app 内 Core 重启后仍保留同一进程关联 |
| 托管实际端口 | UI 确认启动隔离 Python 服务（expected_ports=[]），从真实 OS 快照显示两个 IPv4 端口和一个 IPv6 端口，列表 / 详情 / 端口搜索一致，IPv6 为单层方括号。UI 正常停止后端口移除，外部 1421 仍监听。另一个 PROCESS 就绪、无 TCP 监听的真实服务显示“已就绪”与“未检测到 TCP 监听”及说明，随后正常停止 |
| 错误与只读 | 仅在适配器注入端口读取错误，UI 显示“端口状态未知”，清除监听卡片、禁用关联入口，托管服务仍显示实际服务状态；手动刷新恢复。外部详情仅有归属确认按钮，无启动 / 停止 / 重启 |
| 布局 | 375×640、760×560、1024×768、1380×900 无 document 溢出；760px 服务列表端口可直接看到，详情在自身边界内滚动。375 / 760 / 1380 的归属弹窗在窗口内、确认 / 取消按钮可见。实际回读桌面、多端口、760px 与弹窗截图；375px 仅为额外浏览器边界检查，原生最小窗口仍为 760×560 |
| 打包与签名 | `pnpm tauri build --debug --bundles app` 成功，固定输出 `target/debug/bundle/macos/DevNook.app`（约 42 MiB）；本机 ad-hoc 签名、`codesign --verify --strict --deep` 通过。新包启动隔离 Core，读出 2 项目 / 2 个已正常停止的服务，手动归属持久化、外部来源与监听均正确，随后正常 shutdown |
| 旧包与收尾 | 固定路径由新包替换，未创建验收 .app 副本；扫描项目 target / .tmp 确认仅剩最新 DevNook.app，无旧 dmg / pkg / app.zip。用户原有桌面 / Core 进程未终止。本轮浏览器任务结束、测试服务均正常停止、隔离 Core 经 shutdown 退出，临时 HTTP 适配器与 Web UI 已停止 |

本轮使用 localhost-mcp / ego-browser，在 `.tmp/service-ports-20261113/` 保存隔离数据与验收记录，未写正式数据库、Keychain 或真实 MCP 配置。临时适配器不是产品接口；原生 WebView、Intel / macOS 12 与 VoiceOver 未人工交互复测。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 服务导航与端口状态筛选（2026-11-13）

用户确认：将“工作区组”改名“启动方案”放入服务管理；“发现监听 / 未发现监听”移至项目列表端口状态筛选；验证、重新打包并清理旧包。

- [x] 侧栏移除三个旧入口；服务管理增加“服务列表 / 启动方案”页内导航和面包屑。项目卡片仍进入对应项目的服务列表，切换到启动方案后使用全局服务范围。
- [x] 启动方案页面、表单、保存反馈、启动 / 停止确认及 Agent 授权文案统一；保留 workspace 数据、接口、确认、依赖展开与运行归属规则。
- [x] 项目列表新增“端口状态：全部 / 检测到监听 / 未检测到监听”，与搜索、技术栈、分类及收藏组合。成功 TCP 快照才可筛选；读取失败显示不可用，未知不归为无监听；说明无监听不等于已停止。
- [x] 交互与响应式检查、全量回归、前端检查 / 构建、Debug app 打包、签名及旧产物清理。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 44 passed，0 failed；5 个 ignored 仍仅为被业务测试调用的 fixture。原 Core / 扫描 / 服务 / 工作区 / CLI / MCP 回归通过 |
| 前端检查 | `pnpm check`、`pnpm build`、`pnpm format:check` 通过；本轮没有 Rust 行为变更、依赖或数据库迁移，UDS v7 / SQLite v3 不变 |
| 项目筛选 | 浏览器 React 通过临时适配器连接隔离真实 Core；3 个项目中一个测试 HTTP 进程实际监听 TCP 1421，另外两个无监听。监听 / 无监听、关键词、Node / Java、分类和收藏的交集与空结果逐项通过；侧栏只保留服务管理、CLI 与 MCP、全部项目、已收藏、端口管理 |
| 错误路径 | 仅在测试适配器模拟端口快照读取失败；保留当前筛选并显示“端口状态暂不可用”，两个监听选项禁用，切回“全部”仍显示项目且标为状态未获取。恢复读取后状态自动恢复 |
| 启动方案 | 从服务管理的新入口新建 / 保存“日常开发”，选择前端入口，启动确认实际展开为后端 → 前端；取消确认后无任何运行。服务列表 / 启动方案往返、项目卡片进入服务、切换方案清除项目上下文均通过 |
| 窗口 / 滚动 | 375×720、760×560、1024×768、1380×900 无 document 横向溢出，端口筛选弹层在视口内，启动方案页内滚动、服务列表尺寸正常；实际检查桌面和 760px 截图 |
| 新包 | `pnpm tauri build --debug --bundles app` 成功，固定输出 `target/debug/bundle/macos/DevNook.app`，约 42.15 MiB；本机 ad-hoc 签名与 `codesign --verify --strict --deep` 通过 |
| 包内 Core | 使用新 .app 可执行文件重启隔离 Core，仍能读出已保存方案、3 项目、2 个未启动的服务，方案历史为空；SQLite v3 / UDS v7 未改变，随后经 shutdown 正常退出 |
| 旧包清理 | 标准输出位置由新包覆盖，没有新增验收副本；核对路径 / Bundle ID / 进程后删除前两轮的 Probe.app 和 Remembered Editor.app 共 64 KiB。重新扫描只剩最新 DevNook.app；没有终止用户正在运行的桌面 / Core |

验收数据仅在 `.tmp/navigation-20261113/`；临时 HTTP 适配器不是产品接口。启动方案的 UI 确认流程止于预览取消，真实启停由全量回归覆盖；未在原生 WebView、Intel / macOS 12 或 VoiceOver 上人工复测。浏览器任务已结束，验收服务关闭结果见本段末尾。未操作正式项目库、Keychain、真实 MCP 配置或系统设置，未提交或推送。Ego Lite 提示可更新，本轮未升级。

收尾复核：浏览器已结束，隔离 CLI / 包内 Core 均经 shutdown 退出，测试 HTTP 适配器和 Web UI 已停止；复用的 localhost 验收定义保留供复测。最终签名校验通过；产物扫描确认只保留标准路径的最新 DevNook.app。

## 旧打包产物清理（2026-11-13）

用户要求每次新包验证通过后清理旧包，只保留最新可用包，已写入项目 AGENTS.md。

已核对 Bundle ID、实际路径与进程列表，删除未运行的 `.tmp/simple-services/DevNook-QA.app` 和 `DevNook-QA-Final.app`，清理前 `du -sk` 合计 84,232 KiB（约 82.3 MiB）。保留当前运行的 `target/debug/bundle/macos/DevNook.app`，严格签名检查通过；删除后复核两个旧路径均不存在。未操作运行中的应用、测试数据库、工具链或构建缓存，无代码改动，无需重跑业务测试。

## 打开菜单收紧与手动应用记忆（2026-11-13）

用户确认采用紧凑菜单 + 自适应展开方向；手动选择应用后必须持久加入列表。

- [x] 菜单：默认应用优先；单实例隐藏常驻版本号、重复代码图标与图钉说明，图钉在悬停 / 聚焦时显示；底部选择与刷新合为一行，选项区独立滚动。
- [x] 定位：宽 18rem、最大高 22rem，贴按钮 6px；下方可容纳短列表时优先向下，不够再上翻，箭头同步提示方向。1380×900 的四应用菜单实测从原 360×480 收紧到 252×303，并向下展开。
- [x] 缓存：新增仅本地 remember_development_app，先校验再保存规范路径 / Bundle ID；inspect 保持只读。独立 app_settings JSON 字段不依赖默认偏好，重复选择去重；普通设置保存合并保留缓存，暂不可用不清记录，原路径恢复后重新出现。
- [x] 兼容：无新依赖、无 SQLite schema 迁移（仍 v3）；UDS v7 阻断旧 Core，MCP 不开放记忆接口。文档已同步。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 44 passed，0 failed；5 个 ignored 仍仅为被业务测试调用的 fixture。新增独立记忆回归覆盖不设默认的未知应用、只读 inspect、重复选择、刷新 / 重启、旧设置表单、非法选择、临时不可用 / 恢复、SQLite v3；既有 MCP 隔离测试扩展到新接口 |
| 静态 / 构建 | `pnpm check`、`pnpm check:rust`、`pnpm build:cli`、Prettier / rustfmt 检查通过；Tauri Debug app 构建含 TypeScript + Vite build 通过，约 42.15 MiB，本机 ad-hoc 签名严格校验通过 |
| 前端与真实 Core | 浏览器 React 通过临时测试适配器连隔离 Core；手动选择未知应用后立即入库，刷新可见，项目 / 全局默认均未改变；刷新后键盘导航和 Escape 返回按钮通过 |
| 包内 Core 重启 | 从新 .app 启动 Core 读取同一隔离数据库，手动记录仍只有一条、实际列表再次包含该应用；保存选择之前的旧设置 payload 不清缓存，SQLite user_version=3。v6 被拒绝且 v7 继续运行 |
| 响应式 / 方向 | 375×720、760×560、1024×768、1380×900 均无 document 横向溢出，菜单与固定底栏在视口内，距按钮恒为 6px；前两种向上、后两种向下。实际检查桌面与 760px 截图，1024px 下选项内部滚动 |

临时目录 `.tmp/open-with-compact-20261113/`；未使用正式项目库、Keychain、用户 MCP 配置或系统配置。文件选择器由测试适配器提供已知测试 .app 路径，原生选择器未人工复测；为避免干扰用户编辑器，本轮测试适配器显式拦截真实 IDE 打开，并验证即使打开失败，已经选择的应用仍被记住。未覆盖 Intel / macOS 12 / VoiceOver。Ego Lite 提示可更新，本轮未升级；未提交、推送或公开发布。

验收结束：隔离 Core 经 shutdown 正常退出，Web UI 与临时适配器均已停止，浏览器任务已结束；复用的 localhost 验收定义保留，指向本轮独立目录。用户旧桌面 / Core 未被操作。

## 开发应用发现与打开（2026-11-13）

用户已确认：自动识别本机开发工具，卡片一键打开 / 下拉临时选择 / 独立设置项目默认；Java 新项目优先尊重用户的全局默认设置。

- [x] Core：系统应用登记 + 有限应用目录只读发现、Bundle ID 识别、XML / binary plist 与可执行文件校验、去重、多版本路径和项目推荐。
- [x] 桌面：卡片分体按钮、推荐与目录/终端分组、项目图钉默认、应用刷新、手动 .app 选择、项目 / 设置表单共用检测结果。
- [x] 兼容：保留旧 IDEA / VSCODE / CURSOR、自定义路径和既有档案；新导入 Java 与其他项目都继承全局默认，重复导入不覆盖。
- [x] 安全：发现 / 选择 / 设置不启动应用；打开前重新检查，使用固定 open 参数数组。应用卸载保留偏好并提示；新增本地接口不向 MCP 开放，Core 拒绝 MCP 任意应用路径。

### 实际验证

| 检查 | 实际结果 |
| --- | --- |
| `pnpm test` | 43 passed，0 failed；5 个 ignored 仅为被业务测试单独调用的 fixture。新增 4 个应用 / 默认偏好测试及 1 个 CLI / MCP 边界测试 |
| 新增回归 | Java 继承全局 / 重复导入保留项目偏好；手动应用与带空格路径、重启持久化、卸载保留偏好且打开失败；XML / binary plist、无效清单、缺失 executable、符号链接、非法 target、MCP 本地接口和路径拒绝均通过 |
| 静态检查与构建 | `pnpm check`、`pnpm check:rust`、`pnpm build`、`pnpm build:cli`、Prettier 与 rustfmt 检查通过；无新增依赖、SQLite schema 仍为 v3 |
| 系统真实识别 | 隔离 Core 返回本机 IntelliJ IDEA、Qoder、VS Code、Xcode、Terminal 及实际版本 / 路径；新导入 Java 与 Node 均继承已设置的 Qoder |
| 前端与 Core 联动 | 实际 React 通过仅在 .tmp 内的临时测试适配器调用隔离 UDS Core，完成临时打开不改偏好、图钉默认、项目表单选择保存、全局保存不改已有项目、刷新、缺失应用提示和手动应用回读 |
| 实际打开链路 | 编译无界面的临时 macOS Probe.app，菜单临时选择与默认一键打开均经真实 Core / open 启动，应用收到正确项目目录；没有启动用户真实 IDE 或项目命令 |
| 键盘与窗口 | 刷新后焦点、方向键与 Escape 返回按钮通过；375×720、760×560、768×768、1024×768、1440×900 无 document 横向溢出，菜单在视口内。实际检查 760 / 1440 截图，修正服务文字换行并保持菜单底部操作可见 |
| 最终 Debug 应用 | `pnpm tauri build --debug --bundles app` 构建成功，约 41.98 MiB；本机 ad-hoc 签名与严格校验通过。包内 Core 的复测结果记录在本段末尾 |
| 协议门禁 | UDS 升级 v6；v5 请求明确拒绝，同一 Core 仍能接受 v6。没有自动退出不兼容旧 Core |

验收目录：`.tmp/development-apps-20261113/`，仅使用其中数据库、项目、临时应用和适配器；临时 HTTP 适配器不属于产品，没有增加产品网络接口。原生文件选择器交互由测试适配器替代，仅验证选定路径的 Core 校验和后续动作；未逐一启动所有真实第三方 IDE，未覆盖 Intel / macOS 12 / VoiceOver。Ego Lite 提示可更新，本轮未升级。没有修改正式项目库、Keychain、真实 MCP 配置或系统设置，没有提交、推送或公开发布。

最终包内 Core 复测通过：从新 .app 的可执行文件启动隔离 Core，重读两个项目与全局默认，实际应用发现正常；v5 被拒绝且 v6 PID 不变，SQLite user_version=3。验收 Core 经 shutdown 正常退出；Web UI 与临时适配器已停止，localhost 验收服务定义保留方便复测，用户旧桌面 / Core 未被操作。

## 项目服务交互简化施工清单

用户已确认：项目内直接展示前后端、明确启动命令自动填入、高级设置默认收起；验证导入、启停与日志，并重新打包桌面应用。

- [x] 复用现有服务页面作为项目详情，移出项目编辑表单中的服务候选。
- [x] Core 只读生成启动建议；单行命令解析保持 executable + args，Shell 必须显式开启。
- [x] 默认只显示名称、启动命令和目录提示，高级配置按需展开。
- [x] 隔离环境完成真实导入、启动确认、日志、重启、停止与持久化验收。
- [x] 全量回归、布局检查、文档同步与 macOS .app 重新打包。

范围：不增加依赖或数据库迁移，不自动注册/执行扫描结果，不修改用户项目，不自动合并旧档案，不终止旧 Core，不提交或推送。复用已有配置、版本确认、端口冲突、日志和进程身份核验。

### 本轮实际验收（2026-11-12，macOS arm64）

| 检查 | 实际结果 |
| --- | --- |
| `pnpm test` | 38 passed，0 failed；5 个 ignored 为实际被业务测试调用的隔离子进程 fixture。包含 6 CLI/MCP、8 项目/进程、7 扫描/启动建议、1 JVM、16 服务/工作区测试 |
| 新增回归 | 包管理器继承、脚本变化后重新识别、无运行入口不建议服务、不明确命令留空、符号链接替换不读取；一行命令保留空参数/空格/引号/反斜杠并拒绝隐式 Shell；真实 npm 前后端从导入、保存不执行到确认启动、就绪、日志、重启、停止全流程通过 |
| 前端 / Rust / CLI | `pnpm check`、`pnpm check:rust`、`pnpm build:cli`、`pnpm format:check`、`cargo fmt --all -- --check` 通过；最终 Tauri 打包包含 TypeScript + Vite production build |
| 原生 GUI 导入 | 从独立签名与标识的验收副本打开隔离数据库，真实扫描并只导入 demo-fullstack；扫描和导入后均为 0 个服务配置、0 次运行 |
| 原生 GUI 首次启动 | 点击项目直接看到 frontend/backend/worker；Node 命令自动填入 npm run dev。点击服务行启动，先保存配置，再出现一次简短执行确认。取消 backend 确认后，配置存在但运行历史为空 |
| 原生 GUI 启停 / 日志 | 前后端经 UI 确认启动，均 READY；实际 loopback HTTP 返回 frontend ok / backend ok。UI 显示真实 stdout、心跳及端口日志；backend 经 UI 重启产生第二次运行及新端口；随后经 UI 正常停止 |
| 手动命令与持久化 | 关闭 GUI 后重新打开，项目、配置和日志保留；未识别的 worker 仅补充一行 Python 命令，正常启动并显示 WORKER_READY。首次保存后保持选中 worker；最终经 UI 停止。保留 1 个项目、3 个配置、4 次历史，全部 STOPPED，3 个曾监听端口均已关闭 |
| 安全交互 | 原生表单填写组合命令时明确拒绝，未保存或执行；Shell 未隐式开启。根目录聚合命令位于关闭的“其他启动方式”，测试中未执行；现存服务配置不被启动建议覆盖 |
| 响应式 / 滚动 | 浏览器使用同一原生隔离 Core 的真实快照回放，760×560 / 1024×768 / 1380×900 均无 document 整体溢出；实际滚轮滚动详情时列表及顶部坐标不变，截图已检查 |
| 渐进展开与错误 | 默认可见名称/启动命令两个输入框，高级 details 关闭；项目内所属项目锁定。高级字段无效时，提交自动展开错误字段，最小窗口弹窗自身无额外溢出；无匹配搜索空态正常。项目编辑表单不再显示服务与识别证据 |
| 最终 `.app` | `pnpm tauri build --debug --bundles app` 成功，arm64 Debug `.app` 约 41.23 MiB。初始资源签名检查失败后补齐本机 ad-hoc 签名，`codesign --verify --strict --deep` 通过；重新启动包内 Core 并读取隔离数据、解析单行命令成功 |
| 协议 | UDS 升级 v5，SQLite 保持 v3；v4 请求被拒绝，同一测试 Core 继续运行。未自动终止不兼容旧 Core；原有 MCP 工具与授权回归通过，新内部接口未暴露成 MCP 控制工具 |

原生验收发现并修复首次保存后选中项被旧列表刷新覆盖的问题：保存返回真实配置后同步更新列表，旧读取响应不能覆盖新配置。另锁定项目内新增配置的所属项目，避免标题项目与配置归属不一致。最终包复测首次 Python 服务的选中项、命令参数、日志与停止成功。

验收数据及测试副本仅位于 `.tmp/simple-services/`；原生副本通过 localhost MCP 管理，与用户旧应用使用不同 bundle ID 和数据目录。全部测试服务正常停止，测试 GUI/Core 正常退出，Web UI 已停止，临时改动的 desktop-qa 命令已恢复。没有修改真实项目库、Keychain、用户 MCP 配置、系统设置或用户正在使用的旧应用。Ego Lite 提示可更新，本轮未升级。

剩余边界：未在 Intel/macOS 12、VoiceOver、真实 Keychain 上验收；Maven/Gradle/Rust 建议的识别有清单依据，本轮未启动具体此类工程。建议不保证运行环境或项目依赖已安装，启动失败仍显示实际错误。没有开发者证书签名、Apple 公证或公开发布。

## 扫描发现改进施工清单

用户已确认：扫描结果搜索、默认自动发现、高级深度设置、项目/组件分组、独立嵌套项目导入及桌面重新打包。

- [x] Core：边界识别、分组、服务/共享组件证据、不完整结果提示及只读安全回归。
- [x] 导入：组件线索持久化、去重和已有偏好保留，不自动创建或运行服务。
- [x] 桌面：搜索、分组展开、可见结果全选、选择保留、高级设置和服务候选配置入口。
- [x] 验证：隔离 Core/CLI/MCP 回归、前端类型/格式/构建、浏览器流程及 debug .app 打包。

范围：扫描不读取 .env、不执行项目命令；不迁移数据库、不自动合并或删除已导入档案、不自动关闭旧 Core、不提交或推送。

### 本轮实际验收（2026-11-12，macOS arm64）

| 检查 | 实际结果 |
| --- | --- |
| `pnpm test` | 35 passed，0 failed；5 个 ignored 为业务测试调用的隔离子进程 fixture。覆盖 Core、CLI/MCP、真实服务和工作区回归 |
| 新增扫描回归 | 5 项通过：无根清单 Git 仓库分组、独立嵌套仓库、深层自动发现与不完整提示、workspace/前后端组合、无关目录不误合并、组件持久化、完整刷新/部分合并、旧 JSON/偏好保留、符号链接替换拒绝；旧独立档案保留，未自动配置服务 |
| `pnpm check` / `pnpm check:rust` | TypeScript 和 Rust workspace 通过 |
| `pnpm format:check` / `cargo fmt --all -- --check` | 通过 |
| `pnpm build` / `pnpm build:cli` | 前端 production build 与 CLI 构建通过；无新增依赖 |
| 浏览器交互 | 实际 React 界面使用隔离 Core 扫描/导入结果回放：默认 depth=0；高级 depth=1 显示部分结果；名称/路径/技术栈/组件搜索、清空、无匹配、隐藏选择保留、可见未导入全选/半选、嵌套单选、分组展开、仅导入选定根项目均通过 |
| 浏览器布局 | 760×560 / 1024×768 / 1380×900 无 document/dialog 整体溢出；结果区独立滚动，搜索与底部操作保持固定；实际检查截图 |
| 服务候选交接 | 持久化项目显示 2 个服务候选和 1 个共享组件；点击 frontend 的配置入口预填名称、Node.js 和组件目录，命令保持待配置，未保存或启动服务 |
| `pnpm tauri build --debug --bundles app` | 重新生成 `target/debug/bundle/macos/DevNook.app`，约 40.77 MiB |
| 包内 Core 实测 | 直接运行新 .app 内可执行文件，使用独立数据库：发现 15 个项目，同仓库含 2 个服务/1 个共享组件，嵌套仓库独立；仅导入根项目后持久化 1 个项目、0 个服务。项目脚本未执行，测试 Core 正常退出 |
| 协议兼容门禁 | 包内 UDS v4 请求成功；v3 请求明确拒绝，同一 Core PID 继续运行，不自动终止旧进程。SQLite schema 仍为 v3 |

本轮修正一处既有测试同步：runner 保存终态后还会写退出审计，原测试立即断言内核身份消失存在竞态；改为沿用有限等待，仍核验 runner 实际退出，生产进程行为未改。随后全量测试通过。

扫描自动模式保留安全上限（32 层、100,000 条目、120 秒、5,000 条清单线索）；触达上限或读取失败会提示结果不完整。边界优先使用 Git，其次 workspace，再使用有限的前后端目录组合证据。识别结果是配置线索，不能证明每个组件都可成功运行。

验收数据位于 `.tmp/scan-discovery/` 或测试专用临时目录，未使用正式项目库/Keychain/MCP 客户端配置。浏览器任务已结束，localhost `web-ui` 已停止，隔离 CLI 与包内 Core 已退出；未操作用户正在使用的旧桌面/Core。Ego Lite 提示可更新，本轮未升级。

未覆盖：新扫描流程的原生 WebView 人工操作、VoiceOver、Intel/macOS 12 真机、签名与公证。浏览器回放只验证前端交互；原生包内 Core 通过真实 UDS 单独验收。升级时须退出旧桌面与 CLI/MCP 客户端，按既有流程正常停止托管服务（如有），等待旧 Core 正常闲置退出后再启动新版；应用不会替用户关闭不兼容的旧 Core。

需求来源：`DevPilot-PRD-Technical-Design-V1.0.md`。当前用户指定产品名 DevNook；原始文件保留作为输入资料，不执行其嵌入提示词中的提交等独立外部动作。

## 本轮范围

Phase 0–1：工程初始化、项目扫描预览/取消/导入、SQLite 项目库、IDE 打开、TCP 绑定、进程详情、归属推断与人工修正、安全停止、中文桌面 UI。

## 任务与验收

- [x] Core 扫描器：语言/框架/构建工具、monorepo、跳过目录、符号链接、深度、取消、进度。
- [x] 项目库：规范路径去重、事务导入、收藏/分类/编辑、移除记录、重启持久化。
- [x] macOS 发现：结构化 lsof、IPv4/IPv6、多端口、ps/cwd/executable/父进程链、退出与权限错误。
- [x] 归属与停止：最长 cwd 匹配、命令建议、身份绑定人工修正、同用户与保护进程、一次性确认、SIGTERM、审计。
- [x] IDE：固定应用 + 自定义 `.app` 路径，Finder/Terminal，失败可见。
- [x] 中文桌面：项目多视图、搜索/分类/收藏、端口三栏、自动/手动刷新、设置和真实空态。
- [x] 验证：Rust 集成测试、cargo check、TypeScript、前端构建、macOS 桌面构建与关键路径。

## 风险与取舍

- 原环境没有可用 Rust；已在 `.tmp/toolchain` 安装 stable Rust 1.99.1 与 rustfmt，未修改系统 PATH。开发脚本可优先复用全局工具链。
- 系统隐私权限可能使部分 cwd/executable 不可读；显示未知，不提权。
- 外部来源无法可靠证明来自 Terminal/Codex/IDE；只显示父链线索，统一标记外部发现。
- 首轮 Core 随 GUI；没有托管服务，因此关闭 GUI 不会影响外部进程。后台 daemon 延后至服务阶段。
- 不安装 CI、不发布、不提交，不读取被扫描项目的秘密配置。

## 验证结果

日期：2026-11-12；实际机器：macOS Apple Silicon（arm64）。

| 检查 | 实际结果 |
| --- | --- |
| `pnpm test:core` | 8 passed，0 failed；1 个 fixture 标记为 ignored，由真实进程集成测试作为子进程单独启动，不是未执行的业务测试 |
| `pnpm check:rust` | workspace 通过 |
| `pnpm build` | TypeScript 严格检查与 Vite production build 通过 |
| `pnpm format:check` / `cargo fmt --all -- --check` | 通过 |
| `pnpm tauri build --debug --no-bundle` | 生成本机可执行文件 |
| `pnpm tauri build --debug --bundles app` | 生成 `target/debug/bundle/macos/DevNook.app`，约 34 MiB |
| 原生 GUI 扫描 | 只读扫描本仓库，识别根项目、desktop、src-tauri、devnook-core 共 4 个候选，默认只选根项目，勾选后导入成功 |
| 原生 GUI 项目库 | 收藏、分类编辑、表格视图、重启后恢复项目/收藏/分类已实测；测试数据位于 `.tmp/native-qa`，未污染正式应用数据库 |
| 原生 GUI 端口 | 显示真实 TCP 绑定、PID/PPID/PGID、命令、executable、cwd、父链；本仓库 Vite 进程高置信度关联成功 |
| 原生 GUI 安全边界 | 停止弹窗要求输入 PID，空输入时禁用确认；取消后进程保留；ControlCenter 等保护进程禁止停止 |
| 真实停止测试 | 在隔离目录启动监听 TCP 的测试子进程：未知归属拒绝、cwd 自动关联、人工修正、身份不符拒绝、错误 PID 拒绝、本人保护、正确确认发送 SIGTERM、令牌重复使用拒绝、退出后端口消失 |
| IDE 与错误路径 | 原生 GUI 的 VS Code 打开请求返回成功；不存在的自定义 IDE、非法 target、无效设置和缺失项目路径有测试或显式错误处理 |
| 浏览器布局 | 375 / 768 / 1024 / 1440 宽度无横向溢出；搜索空态、设置页与端口页切换实测。浏览器明确标记预览，禁用原生写入 |

修复的验收问题：扫描刚完成时的取消仍能导入；已改为串行化取消与导入提交，并加入回归测试。PID 身份从秒级 `ps` 时间加强为 libproc 微秒级启动时间，并在发信号前最后复核。项目清单使用 `O_NOFOLLOW` 打开，避免读取指向其他文件的符号链接。

## 明确未覆盖 / 后续

- 尚未在 Intel Mac 或 macOS 12 真机上验收；没有签名、公证或发布。
- IntelliJ IDEA / Cursor 的真实启动，以及不同 IDE 安装布局尚未逐一人工验证；支持设置应用路径。
- 不要求 root，不会绕过系统隐私限制；无法读取精确身份或 executable 的进程禁止停止。
- 首次发现时间仅描述当前 Core 会话；历史监听快照不入库，不作为持久运行事实。
- 没有生产 CI、服务托管、后台 daemon、CLI、MCP、IDEA 插件和工作区编排；按需求阶段推进。
- 本轮无 Git add、commit、push、PR 或部署。原始需求文档及用户 `.idea` 内容保留。
- 通过 localhost MCP 创建了 DevNook 的 Web UI 与 Desktop QA 开发服务，验收结束后均停止；项目/服务定义保留，未删除。
- Ego Lite 提示可更新，本轮未升级浏览器。

## Phase 2：服务托管（2026-11-12）

原 Phase 1 记录保留。当前已实现 Phase 2，详细设计、验收和限制见 [PHASE-2.md](PHASE-2.md)。

- [x] 服务配置与 SQLite v2 增量迁移、v1 备份；每次运行独立历史，归档保留日志。
- [x] executable + args / 显式 Shell Mode、项目内 cwd、普通环境变量与版本化 Keychain 引用。
- [x] 启动/重启前预览、60 秒配置版本确认、重复执行和旧令牌阻断、审计。
- [x] Core 独立后台进程、私有 UDS、同用户权限/单实例锁；runner 脱离窗口生命周期，Core 重启后核验恢复。
- [x] 独立进程组、stdout/stderr 时间戳、脱敏、大小限制、超长行保护、尾部/搜索/历史/单次日志清理。
- [x] 端口冲突阻断与活动运行预留；托管进程按稳定 runner 身份/PGID 识别，外部进程不接管。
- [x] PROCESS / PORT / HTTP / LOG_PHRASE；真实就绪、超时、持续探测与健康失效降级。
- [x] SIGTERM 宽限、进程组后代清理、独立强制确认；不退出则阻止重启，身份不符则 UNKNOWN。
- [x] 中文服务页面、项目入口、配置编辑、确认、状态、运行详情与历史日志。
- [x] 19 项业务测试通过（8 回归 + 10 Supervisor/IPC + 1 JVM）；5 个 ignored 仅为被测试调用的子进程 fixture。
- [x] pnpm check:rust、pnpm build、pnpm format:check、cargo fmt 检查及 macOS debug .app 打包通过。
- [x] 原生 GUI 真实配置/启动/重启/日志；GUI exit code 0 后 HTTP 200 与日志继续；最终测试服务 STOPPED，测试 Core 退出。

所有测试配置和运行数据位于隔离目录；未向正式项目库导入测试服务，未读写用户 Keychain。未提交、推送、发布或安装系统自启动配置。浏览器服务未启动，Desktop QA 已退出；localhost 的服务定义保留。

本阶段原生验收发现并修复 UDS 大响应截断，回归使用超过 8 KiB 的配置验证。另修复 TERM 正常停止收尾时序与探测间隔长于超时的边界。当前未覆盖范围：Intel/macOS 12、真实 Keychain 交互、具体 Maven/Gradle 应用；仅提供 loopback HTTP 健康检查。

## Phase 3：工作区、CLI 与 MCP（2026-11-12）

Phase 0–2 历史记录保留。当前范围与使用方法见 [PHASE-3.md](PHASE-3.md)。

- [x] 跨项目工作区组新建/编辑/入口顺序/移除、启动记录与实时服务状态。
- [x] 服务依赖持久化、确定性拓扑展开、自依赖/重复/缺失/循环检查、单独启动依赖门禁。
- [x] 60 秒计划确认，绑定工作区/服务版本和运行实例；逐个等待 READY，编排过程预留服务，阻止重叠操作。
- [x] 失败/取消逆序回滚，仅停止本次创建的实例，保留既有与后来重启的运行；不自动强杀，失败可见。
- [x] Core 中断恢复：启动前捕获旧任务，按 service_runs.workspace_run_id 找回归属并回滚，不继续未完成启动链。
- [x] clap CLI 项目扫描/明确候选导入/打开、服务控制/日志/历史、端口查询、工作区控制/历史、授权管理；JSON 输出、非交互确认与错误码。
- [x] 标准 stdio MCP 初始化/通知/查询/控制工具，兼容协议版本协商、参数 schema、协议错误与 isError；stdout 不混入诊断。
- [x] 默认只读；Core 端校验范围授权、全部依赖配置版本、令牌归属/动作/对象、撤销状态；限制扫描根目录和扫描会话，导入明确候选须再次确认。
- [x] GUI 授权范围审阅、一次性显示令牌、撤销、最近操作；凭证仅存摘要，审计不存原始请求/环境值/凭证，非 UUID 对象不写审计 subject。
- [x] SQLite v3 事务迁移、v2 备份、旧服务 JSON 默认字段兼容；MVP/架构/技术栈/项目规则/README 同步。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 30 passed，0 failed：8 项目/进程 + 1 JVM + 15 Supervisor/工作区/迁移 + 6 CLI/MCP；5 个 ignored 仅是被测试单独启动的 fixture |
| 工作区真实进程 | Node 延迟日志就绪后下游才启动；复用既有服务、失败回滚、取消、预留、旧确认失效、循环、后来重启保留、正常停止超时可见均通过 |
| Core 中断 | 实际 SIGKILL 隔离测试 Core；清空工作区 owned_run_ids 模拟写记录间隙，重启后仍根据服务归属回滚新增运行，原先运行保留 |
| CLI/MCP | 真实 CLI 子进程与 stdio 客户端；非交互 --yes、共享状态、日志、退出码、只读、越权、撤销、错客户端/错对象确认、配置变化、受限扫描导入与凭证不落库/审计通过 |
| MCP 协议 | malformed JSON、null id、缺失初始化参数、初始化前调用、参数 schema、通知无响应、ping、旧版本输出与未知版本协商通过 |
| `pnpm check:rust` | workspace 通过 |
| `pnpm build` | TypeScript strict + Vite production build 通过 |
| `pnpm build:cli` | 生成 target/debug/devnook；未安装全局命令 |
| Prettier / rustfmt | 格式检查通过，无忽略类型或关闭检查 |
| macOS `.app` | debug 打包通过，约 40.20 MiB；后续 Rust 修改另运行 no-bundle 构建 |
| 浏览器预览 | 独立 Ego task space，工作区/Agent 页面在 375 / 768 / 1024 / 1440 宽度无横向溢出，写操作禁用；桌面/移动截图已检查，未注入样例运行数据 |
| 原生应用 | localhost MCP 启动隔离目录 `.tmp/phase3-native-qa`，确认新导航与 Core 连接；工具随后报告用户正在操作窗口，按规则停止原生自动化，未修改用户当前界面 |

原生工作区表单、启动确认、授权表单的人工交互尚未完成；后端行为已有自动化集成测试覆盖，不将其冒充原生验收。没有向正式数据库导入测试项目/服务，没有创建真实 Agent 授权或修改任何实际 MCP 客户端配置。真实 Keychain、Intel/macOS 12 和外部 MCP 客户端配置 UI 仍未验收。

验收修复：原有后代清理测试在 runner 写 STOPPED 与内核观察最终退出之间直接断言，改成有界等待真实退出；Core 故障测试明确等待 SIGKILL 生效后再启动下一客户端，避免把信号发送当作退出完成。没有对已发送写操作增加自动重试。

浏览器任务已结束，Web UI 开发服务已停止。原生 Desktop QA 因用户接管保留打开，其 localhost 定义使用 Phase 3 隔离数据目录；没有停止用户接管的窗口。Ego Lite 提示有更新，本轮未升级。未提交、推送、发布、安装 LaunchAgent 或修改系统配置。

## 端口页布局调整（2026-11-12）

- 端口页外壳限制在窗口高度内，压缩标题、统计栏和顶部间距；搜索筛选与底部状态固定，列表和进程详情各自按需滚动。
- 移除端口面板、列表和空态原有的固定高度限制，剩余高度交由布局分配；列表行更紧凑，保留置顶列表表头，并支持键盘聚焦、独立滚动与可见焦点。
- 复用 640px 断点覆盖桌面应用的 760px 最小窗口宽度；窄窗口中的侧栏可独立滚动。改动只在端口页启用，项目等其他页面保留原有整页滚动。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 问题复现 | 修改前 1380×900 浏览器预览的文档高度为 1081，端口面板底部超出窗口 |
| `pnpm check` / `pnpm build` | TypeScript 严格检查与 Vite production build 通过 |
| `pnpm format:check` | 全部前端源码通过 Prettier 检查 |
| `pnpm tauri build --debug --bundles app` | 本机 debug 可执行文件及 `target/debug/bundle/macos/DevNook.app` 构建成功，约 40.22 MiB |
| 布局与独立滚动 | 从既有 Phase 3 隔离 Core 只读捕获 68 条真实 TCP 绑定及 Vite 进程详情，在独立浏览器任务中通过临时只读 IPC 适配器回放；760×560、768×700、1024×768、1380×900、1440×900 均无整页横向或纵向溢出，列表和详情分别在自身边界内滚动 |
| 交互与空态 | 实测列表滚轮滚动、详情键盘滚动互不影响，搜索筛选坐标与文档滚动位置保持不变；单端口列表没有多余滚动，搜索无匹配空态和 IPv6 筛选正常；浏览器真实空态截图与真实快照回放截图均已检查 |
| 其他页面 | 切换至全部项目后端口页专用布局类移除，主内容恢复原有 `overflow: visible` 整页滚动 |

本轮只改前端布局与本记录，未修改 Core 或进程控制逻辑，未对用户进程发送控制信号、写入正式数据库或创建 Agent 授权。浏览器回放验证不等于原生 WebView 交互验收；本轮未操作此前由用户接管的原生窗口。验收浏览器任务已结束，本轮启动的 Web UI 服务已停止；原生 Desktop QA 保持运行。Ego Lite 提示有更新，未升级。未提交或推送。

## 项目页布局补齐（2026-11-12）

用户后续截图指出“全部项目”页仍出现整页滚动条。上一轮只覆盖端口页，项目空态仍使用 30–38rem 的最小高度，标题和统计区域也保留原有大间距；1380×900 浏览器预览在项目数为 0 时文档高度仍达 1057。

- 将端口页固定外壳和紧凑顶部扩展为项目与端口共享工作台布局，覆盖全部项目、收藏、发现监听、未发现监听及端口页。
- 项目页搜索、筛选、视图切换、排序和底部状态固定；新增独立项目内容滚动区域，卡片、紧凑和表格视图共用，表格表头在内容区内置顶。
- 空态按剩余空间展开，覆盖原有固定最小高度；缩小插画与文案间距，低窗口高度下收起装饰内容，保留说明和扫描操作。没有通过隐藏滚动条遮盖超出的内容。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 严格检查及全部前端 Prettier 检查通过 |
| `pnpm tauri build --debug --bundles app` | 包含 TypeScript / Vite 前端构建及本机 debug 应用打包，成功生成 `target/debug/bundle/macos/DevNook.app`，约 40.22 MiB |
| 项目空态 | 760×560、768×700、1024×768、1380×900、1440×900、1920×1080 六种窗口，四类项目页共 24 项检查通过；文档宽高等于窗口宽高，空态内容区无多余滚动，底部状态可见；桌面与最小窗口截图已检查 |
| 有数据的三种视图 | 独立浏览器页使用 72 条明确标识为 Layout QA 的临时项目 fixture；六种尺寸 × 卡片/紧凑/表格共 18 项检查通过，内容滚动范围受限于项目区，工具栏和底部状态留在窗口内 |
| 交互与错误状态 | 项目区真实滚轮滚动 500px 后文档滚动为 0，搜索工具栏坐标不变，表格表头保持在项目区顶部；无匹配搜索、技术栈筛选、收藏和监听不可用时的空态检查通过；Core 不可用错误提示保持可见 |
| 端口页回归 | 六种尺寸均无整页溢出，端口面板可用高度均超过 100px；沿用原有列表与详情独立滚动结构 |

本轮只修改 `App.tsx`、`styles.css` 与本记录。项目 fixture 仅存在于独立浏览器页的只读测试适配器中，未写入任何数据库；所有真实本机 Core 调用均被该适配器拒绝，监听数据未使用样例替代。未执行项目打开、编辑、导入、停止进程或授权动作。原生 WebView 本轮未进行交互验收，未提交、推送或发布。

## 桌面下拉框统一（2026-11-12）

将项目技术栈/分类、扫描深度、项目与设置页的默认 IDE、端口项目/地址族/进程归属、服务项目筛选/运行记录、服务编辑器所属项目/运行类型/就绪检测共 13 处下拉框接入共享 `Select.tsx`。展开菜单由应用渲染，沿用现有深色与绿色选中样式，避免 macOS 系统浅色弹出菜单覆盖深色界面。没有新增依赖。

- 统一按钮高度、圆角、边框、箭头与选中勾选标记；长触发文案省略，菜单内完整换行；无可选项目时禁用触发按钮并提供提示。
- 菜单位于页面或最近的原生 `dialog` 内，以固定定位避开内容区裁剪；根据窗口边界向上展开或横向避让，长列表只在菜单内滚动，短列表按内容展开。
- 保留受控选项值和业务回调；方向键跳过禁用项，支持 Home/End、PageUp/PageDown、字符定位及 Enter/Space 确认。Esc、Tab、失焦或外部点击取消未提交的高亮选择；菜单内 Esc 不关闭所在弹窗。键盘语义设计参考 [WAI-ARIA Select-Only Combobox](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/examples/combobox-select-only/)，没有将示例当作完整的辅助技术验收。
- 必填字段保留隐藏的原生 select 执行浏览器表单约束校验，错误聚焦到可见按钮；所有用户可见下拉框均使用共享组件。所属项目表单标签与触发按钮显式关联。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` | TypeScript 严格检查通过 |
| `pnpm format:check` | 全部前端源码通过 Prettier 检查 |
| `pnpm tauri build --debug --bundles app` | 包含 TypeScript / Vite 构建和本机 debug 打包，生成 `target/debug/bundle/macos/DevNook.app`，约 40.22 MiB |
| 共享组件交互 | 独立临时 React 验收页实测方向键跳过禁用项、移动高亮不提交、Esc 取消、Home/End/字符定位与确认、鼠标选择、Tab 取消、空选项/只读禁用、必填阻止提交与选择后提交、外部点击关闭 |
| 弹窗层级 | 使用现有原生 HTML dialog 的 Modal，在裁剪容器内打开菜单；确认菜单仍可见并接收指针、Esc 只关闭菜单、再次选择更新表单值 |
| 长列表与边界 | 60 条长中文选项，375×640、760×560、1024×768、1380×900 四种窗口均保持菜单在窗口内；End 定位最后一项并仅滚动菜单，文档滚动位置保持为 0 |
| 紧凑筛选菜单 | 项目技术栈 7 个选项，在 760×560、1024×768、1380×900 均无菜单内部多余滚动条，页面宽高仍等于窗口宽高 |
| 应用页面 | 实测项目技术栈/分类、端口地址族、设置 IDE 未保存选择及服务项目筛选；使用隔离项目 UI fixture 打开实际服务表单，实测所属项目标签、隐藏必填校验、Python 运行类型及 HTTP 就绪方式选择，关闭表单未保存 |
| 视觉检查 | 项目筛选、弹窗菜单、长列表边缘和实际服务编辑器截图已检查；源码扫描只在共享组件必填校验分支保留隐藏原生 select |

本轮修改 `Select.tsx`、`App.tsx`、`ServicesPage.tsx`、`components.tsx`、`styles.css` 与本记录。测试页与 UI fixture 位于忽略目录和独立浏览器任务，不写真实数据库；所有配置保存、扫描、启动/停止、进程归属写入和授权调用均未执行，Core 和权限策略未改动。只读盘点确认已有 DevNook 原生应用仍在运行，本轮未重启或操作其窗口；原生 WebView 与 VoiceOver 尚未实测。验收浏览器任务与本轮启动的 Web UI 服务均已关闭，Ego Lite 提示有更新但未升级。未提交、推送或发布。

## 服务页高度与信息精简（2026-11-13）

- 服务页移除宣传眉题、大标题说明与重复来源标签；项目上下文、视图页签和项目操作合并为紧凑页头，TCP 时间与刷新合并到搜索工具栏。其他页面保留既有页头。
- 列表压成名称 / 状态、端口 / 项目或相对目录两行，保留一个快捷启停入口，移除与行选择行为相同的日志按钮；外部服务零项不再展示大块说明，未知 / 读取失败继续明确展示。
- 托管详情合并标题、状态与操作；只展示一处实际端口摘要，完整绑定移入运行详情；编辑 / 移除与日志清理分别通过可用键盘操作的更多入口访问。日志使用剩余高度，记录选择、搜索共用一行工具栏，输出区域可聚焦并独立滚动。
- 草稿改用“配置启动方式”标题，减少服务名重复，压缩表单及底部留白；沿用现有主题语义变量。未变更 Core、配置模型、协议、命令解析、版本确认、控制调用或权限边界。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm build` / `pnpm format:check` | TypeScript、Vite production build 与全部前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | 成功生成 `target/debug/devnook-desktop`，包含最新前端资源；未替换运行中的 `.app` |
| 浅色 / 暗色布局 | 复用此前真实隔离数据库快照，浏览器注入只读回放适配器；全局服务页覆盖 760×560、1024×768、1380×900、1920×1080，项目服务页再覆盖两主题 × 四尺寸，均无整页横向 / 纵向溢出。主题切换和浅暗截图已检查 |
| 内容高度 | 760×560 工作区起点约 154px，1380×900 约 157px；1380×900 日志约 517px 高，占右侧约 71%，日志内部 PageDown 后 scrollTop 为 558、详情与文档滚动均为 0；项目服务列表行约 65px |
| 日志与更多入口 | 搜索 / 清空、历史记录选择、日志更多入口可见与外部点击关闭、服务更多入口 Tab 聚焦 / Escape 关闭并回到触发项、编辑入口打开原表单均通过；草稿页在 760×560 可见提交按钮 |
| 状态与确认 | 仅在隔离浏览器内模拟 READY / UNKNOWN / STOPPING / CRASHED，检查对应操作可见性 / 禁用、错误信息及端口无监听的独立语义；启动 / 重启预览、强停需输入进程组 ID 且确认按钮初始禁用、端口读取失败显示未知均通过；所有确认均取消，未发起实际执行 / 停止 / 保存 / 清理请求 |
| 调用一致性与回归 | TypeScript AST 对比确认服务页 12 处 Core 调用及参数表达式保持一致；启动方案空态、搜索无匹配、展开运行详情和小窗口内部滚动均已检查 |

历史快照及 UI 状态模拟仅用于布局和入口验收，不作为当前本机进程事实；没有接入真实 Core 或写入正式数据库，没有对任何真实服务发送控制信号。原生构建通过不等于原生 WebView 交互验收，本轮未操作用户现有窗口。盘点确认固定路径旧 `.app` 的 GUI、Core、worker 仍在运行，按项目规则保留，待用户正常退出后再更新应用包及清理旧副本；没有生成额外 `.app` 或停止旧进程。浏览器任务和本轮 Web UI 服务在验收后关闭。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 服务布局原生预览包（2026-11-13）

- 按用户要求重新执行 `pnpm tauri build --debug --no-bundle`，再将最新可执行文件交给 Tauri bundler，使用固定的 `target/preview/debug/bundle/macos/DevNook.app` 输出独立预览包（约 44.07 MiB）；原 `target/debug/bundle/macos/DevNook.app` 未覆盖。
- 预览包补齐本机 ad-hoc 资源签名后，`plutil -lint` 与 `codesign --verify --deep --strict` 均通过。通过系统 `open -n` 打开，确认新 GUI 进程来自预览路径，并在原生 WebView 看到紧凑服务页与真实服务列表。用户操作窗口后停止界面控制。
- 打开前后只读比较现有 Core 返回的服务运行记录、状态、PID / PGID、runner 微秒启动身份、配置版本与 workspace_run_id，均保持一致；Core PID 65870、runner PID 95832 和服务 PID 95836 的启动时间及命令也一致。`backend` 继续 READY，Core 核验的实际 TCP 监听仍为 `:5001`。
- 未发送任何服务控制信号或调用配置写入；未退出、重启或替换 Core / runner。核对时旧 GUI 已退出，但旧包仍被后台 Core / runner 使用，因此按规则保留；新版窗口保持打开供用户查看。打包中间的未运行可执行副本已核对并清理，没有累积额外验收包。

## 外部服务详情精简（2026-11-13）

- 仅调整 `ServicePorts.tsx` 与相关样式：标题旁集中监听状态和项目归属按钮，只读标识、项目、PID 合并一行；监听地址 / 协议族、命令、目录改为紧凑标签和值，移除两层卡片、重复 PID 和解释段落。
- `ListeningPorts` 增加仅供外部详情使用的紧凑展示，原默认模式继续用于托管运行详情和项目归属弹窗；清理由本次移除卡片产生的无引用样式。沿用语义主题变量，不修改 Core 或服务操作。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 严格检查、全部前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | TypeScript / Vite production build 和原生可执行文件编译通过，未覆盖或重启正在运行的应用包 |
| 浅暗布局 | 从现有 Core 只读捕获真实外部服务 `node server.mjs` 的监听与归属数据，在隔离浏览器回放；两主题 × 760×560 / 1024×768 / 1380×900 共 6 项检查无整页或详情横向溢出，详情内容高约 145–163px；桌面和最小窗口截图已检查 |
| 信息与归属入口 | 详情 PID 仅出现一次，完整地址、协议族、命令与目录保留；唯一操作为项目归属。实测打开原归属弹窗、显示原指纹 / 只读说明与非紧凑监听信息、Escape 关闭；未提交关联，适配器未收到任何写入或进程控制调用 |
| 逻辑与服务保全 | TypeScript AST 确认 `ProcessAssignment` 完整逻辑保持一致；修改前后只读比较 Core 的服务运行记录、状态、PID / PGID、runner 微秒启动身份、配置版本及 workspace_run_id 均一致；Core、runner、托管服务与观察的外部服务进程 PID / 启动时间 / 命令均未变化，`backend` 仍 READY |

验收数据与只读适配器仅在忽略的临时目录及独立浏览器任务中使用；未写入真实项目归属、配置或数据库，未操作现有原生窗口，未向服务发送信号。本轮只编译新可执行文件，正在运行的 `.app` 保持原样，尚未包含本次外部详情调整。验收浏览器和本轮启动的 Web UI 服务已关闭。未提交、推送或发布。

## 侧栏精简与新版打开（2026-11-13）

- 移除“我的工作台 / Personal workspace”、工作空间分组标题、Logo 英文副标题和底部宣传卡片；同步清理由本次删除产生的无引用样式。
- 主导航改为全部项目、已收藏、服务管理、端口管理，保留项目 / 收藏数量，取消侧栏端口计数；CLI 与 MCP 移至底部设置旁。底部状态简化为后台连接文案，保留原连接判断、快捷键、选中态和浅暗主题；版本信息继续在设置关于区域显示。
- 修改范围为 `App.tsx`、`styles.css`、`theme.css` 及设计 / 验证记录，未改 Core、服务管理或端口归属逻辑。新版也包含上一轮已完成的外部服务详情精简。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 严格检查、全部前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | TypeScript / Vite production build 与原生可执行文件编译通过 |
| 浅暗主题与尺寸 | 隔离浏览器回放真实项目 / 监听快照，两主题 × 760×560 / 1024×768 / 1380×900 共 6 项检查通过；文档无整页溢出，侧栏无多余滚动，底部连接状态始终在窗口内，主导航与工具导航顺序正确；浅暗桌面及最小窗口截图已检查 |
| 导航与连接 | 实测四个主导航选中态 / 页面切换、Tab + Enter 导航、设置选中与关于版本可见；模拟连接失败时显示“后台未连接”、灰色状态点及数量 `—`，手动重连恢复“后台已连接”；无真实配置写入或服务控制请求 |
| API 一致性 | TypeScript AST 比较确认 `App.tsx` 的 14 处 Core 调用及参数表达式保持一致 |
| 打包与原生窗口 | 使用固定 `target/latest/debug/bundle/macos/DevNook.app` 输出约 44.07 MiB 的新包，补齐本机 ad-hoc 资源签名，`plutil -lint` 与 `codesign --verify --deep --strict` 通过；系统 `open -n` 打开后，确认新 GUI 来自最新包路径，原生窗口真实项目列表、简化侧栏、导航顺序与后台连接状态均已观察，保留窗口供用户查看 |
| 服务保全 | 打开前后服务运行记录、状态、PID / PGID、runner 微秒启动身份、配置版本、workspace_run_id 完全一致；Core、runner、服务及实际监听子进程的 PID / 启动时间 / 可执行文件均未变化，`backend` 仍 READY，Core 核验监听仍为 `:5001` |

本轮未退出、重启或覆盖现有 Core / runner 或应用包，未对用户服务发送信号。原 `target/debug/bundle/macos/DevNook.app` 仍供后台使用，旧 `target/preview/debug/bundle/macos/DevNook.app` GUI 仍在运行，按规则保留至正常退出后清理；新包的未运行打包中间副本已核对并清理。隔离浏览器与本轮启动的 Web UI 服务已关闭，最新原生窗口保持打开。未提交、推送或发布。

## 紧凑项目列表与分类分组（2026-11-13）

- 项目默认紧凑列表，压缩行内留白并保留项目、路径、技术栈、监听状态及原操作；卡片和表格共用原项目渲染与控制入口。
- 增加“不分组 / 按分类分组”，对筛选后的结果分组并显示数量；组可通过按钮或键盘折叠，未分类置底、不出现空组或重复项目，分组行不重复分类名。无任何分类时保持平铺。搜索自动展开匹配组，清除后恢复平时的折叠选择。
- 项目编辑使用已有分类下拉，可新建分类或设为未分类；名称去除首尾空白，并按现有 Core 的 80 UTF-8 字节限制校验。分类仍保存在原 category 字段，不新建分类表、不增加侧栏树。分类过滤使用独立内部值，避免名称 ALL 等与筛选哨兵冲突；最后一个成员移出当前分类后恢复全部分类。
- 新增 `ProjectPresentation.ts` 与 `CategoryPicker.tsx`；展示偏好以版本化 localStorage 保存视图、分组、排序和折叠状态，搜索和筛选不持久化。损坏数据恢复默认；存储失败保留本次交互并提示。更新 README、MVP、架构、技术栈与设计说明；无依赖、Core、协议、数据库 schema 或服务配置变更。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 严格检查、前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | TypeScript / Vite production build 与原生编译通过 |
| Core 隔离持久化 | `cargo test -p devnook-core --test core persistence_dedup_edit_and_remove_preserve_source -- --exact` 通过（1 passed）；测试使用 TempDir，分类在数据库重开后保留，移除档案不删除源目录 |
| 分组与筛选 | 96 条明确标识为 Layout QA 的项目元数据 fixture 分为 6 组，每组 16、总数及唯一 ID 均为 96；收藏 24、技术栈 / 分类组合空态、字面分类 ALL 筛选 16、基于真实捕获监听的端口筛选 1 均通过；无分类时显示 96 条平铺项目 |
| 分类编辑 | 隔离浏览器内存适配器通过实际表单验证已有分类、新建、首尾空白清理、必填阻断、中文超 80 字节阻断、设为未分类，以及分类消失后的筛选恢复；没有向正式数据库提交编辑 |
| 展示偏好与键盘 | 默认紧凑、不分组、最近添加；切换到表格 / 分类分组 / 名称排序并折叠后重载全部保留；搜索暂时展开、清空恢复原折叠；Space / Enter 可切换组；损坏 JSON 降级、存储读写失败提示且仍可切换布局通过 |
| 浅暗主题与尺寸 | 两主题 × 760×560 / 1024×768 / 1380×900 × 紧凑 / 卡片 / 表格共 18 项检查通过；文档无多余溢出，底部状态可见，项目内部滚动。浅暗桌面 / 最小窗口与窄窗口新分类编辑截图已检查 |
| Core 调用一致性 | TypeScript AST 对比确认 `App.tsx` 原 14 处 Core 调用与参数表达式保持一致；未改变服务启停、监听或项目归属逻辑 |
| 打包与原生观察 | 上轮 preview GUI 已正常退出，核对路径及无运行引用后清理旧 preview 包；固定 `target/preview/debug/bundle/macos/DevNook.app` 生成约 43.83 MiB 新包，ad-hoc 签名、`plutil -lint`、`codesign --verify --deep --strict` 通过。系统 `open -n` 打开，新 GUI 来自 preview 路径；原生截图和辅助功能树确认 102 个真实项目、默认紧凑、分组入口、浅色主题与后台已连接 |
| 服务保全 | 打开前后服务运行 ID、READY、PID / PGID、runner 微秒身份、配置版本及 workspace_run_id 完全一致；Core、runner、npm 服务与监听子进程 PID / 启动时间 / 可执行文件不变，Core 核验的 DEVNOOK 监听仍为 `:5001` |

验收浏览器任务及本轮启动的 Web UI 已关闭；最新原生窗口保留供用户查看。真实项目的分类没有被自动分配或修改；分类写入交互为内存隔离验收，持久化由 Core 的临时数据库测试验证。原 `target/debug/bundle/macos/DevNook.app` 仍供 Core / runner 使用，`target/latest/debug/bundle/macos/DevNook.app` 旧 GUI 仍在运行，因此保留这两份包，待正常退出后再清理；新包未运行的打包中间副本已核对并移除。未停止、重启或覆盖现有 GUI / Core / runner，未向用户服务发送信号。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 多选项目批量分类（2026-11-13）

- 结果栏增加“批量分类 / 退出多选”，紧凑、卡片与表格共用项目复选框；可全选当前筛选结果、选择整个分类组，部分选择显示半选状态，折叠不丢失选择。新建分类、已有分类及设为未分类复用原 CategoryPicker；未选择目标时不允许保存。
- 勾选与批量目标仅保存在当前界面。搜索 / 筛选切换清空勾选，实时快照或项目编辑导致项目退出当前结果时取消其勾选；导航离开项目页退出多选。多选时收起统计条、全选合并到结果栏，批量栏有高度上限，错误或新分类输入保留项目滚动空间；退出恢复原浏览布局。浅暗主题沿用语义变量。
- `ProjectBatch.ts` 仅串行调用已有 list_projects / edit_project：每项保存前重新读取项目，保留当时的名称、默认应用与收藏，仅替换分类；ID 去重，同分类跳过写入，首个错误停止且不自动重试。UI 显示进度、锁定重复提交和项目编辑 / 导航，成功项取消勾选，未完成项保留并提示核对后重试。逐项目持久化，未引入跨项目事务或并发隔离保证。
- 修改 `App.tsx`、`CategoryPicker.tsx`、`styles.css`，新增 `ProjectBatch.ts` / `ProjectSelection.tsx`，同步 README、设计与架构说明。Core、UDS v10、数据库、依赖、服务控制及主题偏好实现保持不变。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 与前端 Prettier 检查通过 |
| 原生构建 | `pnpm tauri build --debug --no-bundle` 通过，包含最终 TypeScript / Vite production build 与原生编译 |
| 保存逻辑定向检查 | 用 TypeScript 编译后的实际 `ProjectBatch.ts` 配合内存调用适配器，验证 ID 去重、保存前重读并保留当前其他偏好、串行写入、同分类不写入、首错停止、不存在项目阻断、UTF-8 80 字节上限及清空分类，均通过 |
| Core 隔离持久化 | `cargo test -p devnook-core --test core persistence_dedup_edit_and_remove_preserve_source -- --exact` 通过（1 passed，8 filtered）；使用临时数据库验证项目编辑、分类持久化与源码保全 |
| 浏览器实际交互 | 96 条 Layout QA 项目 fixture，批量分类 / 全量清空分类通过；逐项勾选、全选、整组选择、半选、折叠保留、Space 清空全选及筛选清空勾选通过。未选目标禁用、空新分类必填、中文超限阻断均通过；六项保存第二项模拟失败后已完成 1、其余 5 保留选择，显式重试成功。保存中搜索及退出多选禁用，退出后统计条恢复 |
| 浅暗主题与尺寸 | 两主题 × 760×560 / 1024×768 / 1380×900 × 三种视图共 18 项通过，文档无多余溢出、批量栏无横向溢出、底部状态可见，列表最小高约 165.6px；760×560 下新分类和失败提示时列表仍约 128.4px。浅暗与窄窗口截图已检查 |
| 原有调用一致性 | TypeScript AST 比较确认 `App.tsx` 原 14 处 Core 调用及参数表达式保持一致；新增保存逻辑只使用原项目读写接口，没有服务控制调用 |
| 打包与原生窗口 | 两份旧 GUI 和原后台包仍运行，未覆盖；新包使用固定 `target/current/debug/bundle/macos/DevNook.app`，约 43.83 MiB。ad-hoc 签名、`plutil -lint`、`codesign --verify --deep --strict` 通过。系统 `open -n` 打开，GUI 进程来自 current 路径；原生截图与辅助功能树确认真实 102 个项目、后台已连接、多选栏、项目复选框和未选择时禁用保存，界面留在多选供用户查看 |
| 服务保全 | 修改前后只读比较运行 ID、READY、PID / PGID、runner 微秒身份、配置版本与 workspace_run_id 全部一致；Core、runner、npm 服务及监听子进程 PID / 启动时间 / 可执行文件未变，Core 核验的 DEVNOOK 监听仍为 `:5001` |

批量写入验收仅使用浏览器内存数据，原生窗口只进入多选、没有保存真实分类或操作用户项目 / 服务。浏览器任务与本轮 Web UI 已关闭，未运行的打包中间副本已核对并移除；原 `target/debug` 后台包、`target/latest` 和 `target/preview` 旧 GUI 包仍有运行引用，因此保留，待正常退出后清理。没有退出、重启或覆盖这些进程和应用包，没有向用户服务发送信号。Ego Lite 提示更新，未升级。未提交、推送或发布。

## 端口页统计口径与筛选数量（2026-11-13）

- 侧栏端口管理继续不显示数量。端口页隐藏原项目库统计条，把当前结果 / 完整快照总数、最近成功读取时间和统计口径集中到筛选下方；列表标题与底部不再重复计数和时间。返回项目页恢复原统计条。
- 数量沿用原列表 PID + 端口合并规则，IPv4 / IPv6 和多个地址计为同一监听项，包含系统与开发工具。搜索、项目及地址族筛选只改变分子；总数从完整成功快照计算。刷新中保留上次有效快照并注明状态；未连接、未读取或读取失败为“—”，成功空快照为 0。
- 仅修改 `App.tsx`、`styles.css`、README、设计与本记录；未改变列表筛选 / 去重、进程详情、归属确认、停止确认、Core 或服务控制逻辑，无依赖 / 协议 / 数据库变更。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 与前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | 最终 TypeScript / Vite production build 与原生编译通过 |
| 真实快照回放 | 通过只读 Core 调用捕获 67 条绑定，在隔离浏览器回放为 61 个 PID + 端口项；初始 61 / 61，端口搜索 1 / 61，无匹配 0 / 61，IPv6 18 / 61，叠加未关联项目 15 / 61，分母与列表行数一致 |
| 状态与更新时间 | 模拟刷新延迟时保留 61 / 61 和上次成功时间，并显示正在刷新；模拟读取失败为“— / —”且无成功时间；成功空快照为 0 / 0 并具有有效时间，恢复后为 61 / 61。适配器只接收主题与项目 / 设置 / 应用 / 监听读取，没有写入或进程控制调用 |
| 浅暗主题与尺寸 | 两主题 × 760×560 / 1024×768 / 1380×900 共 6 项通过：文档无多余溢出，摘要可见、列表无横向溢出，工作区最小高约 277.7px；浅色桌面与暗色最小窗口截图已检查。侧栏无端口数字，返回项目页统计条恢复 |
| 原有调用一致性 | TypeScript AST 对比确认 `App.tsx` 原 14 处 Core 调用及参数表达式一致 |
| 打包与原生观察 | 原 latest GUI 已正常退出，核对无运行引用后更新固定 `target/latest/debug/bundle/macos/DevNook.app`；ad-hoc 签名、`plutil -lint` 与 `codesign --verify --deep --strict` 通过。系统 `open -n` 打开，新 GUI 确认来自 latest 包；原生辅助功能树及截图确认端口页真实 61 / 61、更新时间、统计口径与侧栏无数量，窗口留在端口页供用户查看 |
| 服务保全 | 修改前后只读核对服务运行 ID、READY、PID / PGID、runner 微秒启动身份、配置版本和 workspace_run_id 完全一致；Core、runner、npm 服务与监听子进程 PID / 启动时间 / 可执行文件不变，DEVNOOK 核验监听仍为 `:5001` |

浏览器错误 / 空态仅为隔离验收，不作为本机运行事实；没有写入真实项目库、确认归属或向用户服务发送信号。浏览器任务及本轮 Web UI 已关闭；新包未运行的中间二进制与已退出的旧 preview 包在核对字面量路径和进程引用后清理。原 `target/debug` Core / runner 包及 `target/current` 旧 GUI 仍运行，保留至正常退出后清理，未覆盖或终止。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 端口页顶部压缩（2026-11-13）

- 端口页移除英文眉题、LOCAL FIRST 标记、装饰圆点与重复简介；其他页面的标题与宣传标记保持原样。紧凑标题、结果数量、更新时间和刷新共用页头，搜索与两个筛选共用下一行，保留一行 PID + 端口统计口径。
- 搜索与下拉控件统一为 2.4rem 高；端口工具栏使用三列网格，清理由原布局产生的无效 flex 断点规则与摘要容器样式。保留语义主题变量、可见焦点、响应式换行及列表 / 详情内部滚动。
- 本轮仅修改 `App.tsx`、`styles.css`、DESIGN 与本记录，未改动 Core、统计 / 筛选规则、刷新回调、详情与归属 / 停止确认流程；保留工作区中已有的高级接入调整，无新增依赖或业务设置。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| TypeScript / 格式 / 构建 | `pnpm check`、`pnpm format:check`、最终 `pnpm tauri build --debug --no-bundle` 均通过，包含 Vite production build 和原生编译 |
| 调用与表达式一致性 | TypeScript AST 对比确认原 14 处 Core 调用、参数表达式，以及 visible / rows / total / hasSnapshot 表达式与修改前一致 |
| 浅暗主题与尺寸 | 两主题 × 760×560 / 1024×768 / 1380×900 共 6 项通过：文档无整页溢出，搜索与两个筛选同一行对齐，页头无横向溢出，英文眉题和宣传标记不再出现；浅色最小窗口与暗色桌面截图已检查 |
| 高度比较 | 对照上一轮端口页布局记录，760×560 工作区起点从约 268.3px 提前到 166.3px，节省约 102px；1024×768 / 1380×900 从约 245.1px 提前到 169.8px，节省约 75.3px |
| 操作与状态 | 使用新捕获的真实监听 / 项目 / 进程详情只读回放，验证 Cmd/Ctrl + k 聚焦搜索、端口搜索 1 / 61、IPv6 18 / 61、叠加未关联项目 15 / 61、原进程详情与停止入口仍存在；刷新中保留原时间并禁用刷新，读取失败显示“— / —”，760×560 下无溢出，恢复后为 61 / 61。适配器只接收读取与主题调用，没有服务控制 / 归属写入 |
| 打包与原生观察 | 固定 `target/preview/debug/bundle/macos/DevNook.app` 重新打包，ad-hoc 签名、`plutil -lint`、`codesign --verify --deep --strict` 通过；系统 `open -n` 打开，进程来自 preview 路径，原生辅助功能树及截图确认紧凑标题、61 / 61、更新时间、搜索 / 筛选与统计口径，窗口留在端口页 |
| 服务保全 | 修改前后只读核对运行 ID、READY、PID / PGID、runner 微秒启动身份、配置版本及 workspace_run_id 完全一致；Core、runner、服务和真实监听子进程的 PID / 启动时间 / 可执行文件未变，Core 核验 DEVNOOK 监听仍为 `:5001` |

浏览器与本轮启动的 Web UI 已关闭，未运行的打包中间副本已核对并清理。原 `target/debug` 后台包、`target/current` 与 `target/latest` 旧 GUI 仍有运行引用，按规则保留，未退出、重启或覆盖。未写入真实项目 / 服务配置，未确认归属或发送进程控制信号。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 通用桌面尺寸与控件密度（2026-11-14）

- 默认窗口从 1380×900 调整为 1280×800，最小窗口保持 760×560。只调整 Tauri 窗口尺寸，不增加窗口持久化、Core 设置或其他启动行为。
- 共享样式定义侧栏 216px、导航行高 36px、常规控件 32px、紧凑按钮 28px；顶栏统一为 48px。页面上下留白 16px，水平留白在 768px 以下为 16px，其他支持尺寸为 24px；移除随大屏继续放大的侧栏和外边距覆盖。
- 项目、端口、服务、启动方案、设置及弹窗复用尺寸变量；对齐搜索、下拉、表单、视图切换、主题入口和服务工具栏。设置在最小窗口保留左侧导航与底部连接状态，不再切换成顶部导航。各组件字号、字体、语义颜色和服务控制逻辑保持不变。
- 本轮修改 `tauri.conf.json`、`styles.css`、`theme.css`、DESIGN 与本记录。沿用工作区中并行发生的服务浏览器入口调整，不覆盖其他工作。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 与全部前端 Prettier 检查通过 |
| `pnpm tauri build --debug --no-bundle` | 最终 TypeScript / Vite production build 和原生可执行文件构建通过；输出为 `target/debug/devnook-desktop` |
| 字体保全 | 与本轮修改前副本逐项比较，`styles.css` 的 282 条及 `theme.css` 的 4 条字体声明完全一致；根字号实测保持 14px |
| 浅暗主题与窗口尺寸 | 独立 Ego 浏览器任务使用内存隔离的 UI fixture，浅色 / 暗色 × 760×560 / 1024×768 / 1280×800 / 1920×1080 × 项目三种视图 / 端口高级筛选与进程详情 / 服务详情及长日志 / 设置，共 48 项通过。工作台无整页横向或纵向溢出，设置允许正常页面滚动；列表、详情和日志无横向溢出 |
| 统一尺寸 | 各尺寸实测侧栏 216px、导航 36px、顶栏 48px，水平留白分别为 16px / 24px，按钮和输入 / 下拉控件为 28px 或 32px。侧栏无内部溢出，底部连接状态在窗口内 |
| 截图检查 | 已检查浅色默认窗口项目列表、暗色最小窗口端口高级筛选和详情、浅色最小窗口服务详情及暗色最小窗口设置的真实 React 渲染截图 |
| 弹窗、菜单与滚动 | 两主题 × 760×560 / 1280×800，项目编辑 / 添加服务弹窗、长日志滚动、启动方案共 16 项补充检查通过。弹窗与菜单在窗口内，控件为 32px；项目分类支持键盘打开和 Escape 先关闭菜单、再关闭弹窗。日志滚轮后 scrollTop 为 253px，详情与文档均为 0；未保存表单。浅暗小窗口弹窗截图已检查 |

验收 fixture 与展示偏好只存在独立页面内存，未接入真实 Core 或数据库，没有保存项目 / 服务配置或调用进程控制，页面无运行时错误，独立浏览器任务已关闭。本轮复用已有 Web UI 服务，未启动、停止或重配该服务；仅构建原生可执行文件，未替换现有 `.app`、操作用户窗口或新建打包副本。原生 WebView 的布局与新默认窗口开启效果尚未实测。浏览器临时资料位于忽略目录 `.tmp/window-sizing-20261114/`，Ego Lite 提示更新但未升级。未提交、推送或发布。

## 中英界面与系统语言默认值（2026-11-14）

- 首次启动由独立只读 Tauri 命令 `system_language` 读取 macOS 首选语言；只检查第一项，`zh` 及其地区 / 脚本变体选择中文，其余选择英文。浏览器预览使用 `navigator.language`，读取失败默认英文。本机显式 `devnook.language` 优先；自动识别不写成手动偏好，非法存储值按系统默认降级。
- 设置首项增加“界面语言 / Language”，提供简体中文与 English，立即生效并保存到本机，无需提交设置表单。存储失败时仍切换本次语言并显示提示，Core 断连时仍可使用。
- 集中词典当前包含 818 条界面及已知诊断文案，覆盖导航、项目、扫描、端口、服务、启动方案、CLI / MCP 接入说明、选择菜单、确认、校验和辅助标签。日期、时间及显示排序随语言更新；文档 lang 和标题在首屏渲染前设置。
- React 语言订阅只触发重绘，不重新挂载页面或表单。项目名、分类值、路径、命令、用户输入及原始日志保持原文；已知 Core 诊断仅在展示时翻译，未知底层诊断保留内容。原有业务请求及参数表达式保持一致，不改变 Core、UDS v10、SQLite、权限或服务生命周期。
- 原生读取直接复用已锁定的 `objc2-foundation` 0.3.2；Cargo.lock 仅增加桌面包的依赖关联，无新增锁定包或 npm 依赖。更新 README、MVP、架构、技术栈、设计、第三方许可及项目规则。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| `pnpm check` / `pnpm format:check` | TypeScript 与全部前端 Prettier 检查通过 |
| `pnpm test:ui` | 11 项通过，包含中文语言变体、非中文默认英文、手动覆盖、重开持久化、存储失败、翻译插值与参数保全、词典完整性、旧 WebView 缺少 Object.hasOwn 时的启动翻译，以及中英文未关联项目搜索 |
| `pnpm test` | Rust 工作区全量测试通过；5 个隔离 fixture 入口按既有规则忽略，CLI/MCP、进程身份、端口冲突、服务和工作区回滚等测试保持通过 |
| `pnpm check:rust` / `cargo fmt --all --check` | 工作区 Rust 检查和格式检查通过 |
| `pnpm tauri build --debug --no-bundle` | 最终 Vite production build 与原生构建通过，输出 `target/debug/devnook-desktop`；前端 JS 451.49 kB / gzip 141.23 kB，无新增前端依赖 |
| 系统默认与重开 | 在独立域名的真实 React 页面中模拟原生首选语言，6 个系统语言场景及 2 个手动覆盖重开场景通过：zh-CN / zh-Hant-TW / zh_HK 为中文，en-US / ja-JP / fr-FR 为英文；日语首选、中文次选仍为英文。实际 localStorage 保留手动选择并覆盖相反系统语言 |
| 双语 / 主题 / 尺寸 | 中英 × 浅暗 × 760×560 / 1024×768 / 1280×800 × 项目三视图 / 端口高级筛选与详情 / 服务详情及长日志 / 设置，共 72 项通过。工作台无整页溢出，设置可正常纵向滚动；侧栏 216px、根字号 14px，底部连接状态可见；英文页面排除用户内容与原始日志后无遗留中文界面文案 |
| 表单与降级 | 7 项补充交互通过：双向切换后未保存的刷新间隔仍为 7；存储写入失败时界面转为英文、原手动选择保留并显示失败提示；服务编辑器切换后名称与启动命令保持；CLI / MCP 展开、小窗口空扫描、启动方案与后台断连时语言切换均通过。未提交设置或服务表单 |
| 最终文案及弹窗 | 英文最小窗口复查计数为 Results 4 / total 4，外部服务标题 48px 高且无横向溢出；端口冲突诊断正确显示英文。项目编辑、添加服务、扫描、设置及浅暗主要页面截图已检查；中文目录与分类、原始中文日志仍保留原文 |
| 调用与样式保全 | TypeScript AST 比较确认原有 Core 调用及参数表达式无变化；新命令仅供 GUI 读取语言。共享 styles.css / theme.css 与本轮修改前一致，延续 1280×800 默认窗口、既有字号和主题语义 |

界面验收的 Core / 原生语言响应使用隔离适配器，展示偏好使用专门的 localhost 子域名与真实 localStorage，不接入正式数据库，不修改系统语言、真实 MCP 客户端配置或创建真实范围授权。现有 Rust 测试仅使用其隔离 fixture；未对用户服务发送启停请求。浏览器任务和本轮通过 localhost 启动的 Web UI 已关闭，未替换现有 `.app`、重启用户 Core / runner 或生成额外打包副本。原生 WebView 手动交互、真实系统语言切换及旧 macOS / Intel 实机尚未实测；原生 API 绑定与完整应用构建已通过。实验脚本和截图位于忽略目录 `.tmp/i18n-20261114/`。Ego Lite 提示可更新，未升级。未提交、推送或发布。

## 最新原生包与语言验收（2026-11-14）

- 按用户要求重新构建并打开固定路径 `target/latest/debug/bundle/macos/DevNook.app`。开工先记录实际 Core、runner、托管服务和监听身份，确认 latest 路径无运行引用后才替换包；不覆盖正在被 Core / runner 使用的 `target/debug/bundle/macos/DevNook.app`。
- 本轮不修改功能代码。通过原生辅助功能树、截图和本机 Web Inspector 检查真实打包 WebView；没有使用浏览器 fixture 替代原生验收。语言切换只操作界面偏好，未保存业务设置、项目或服务配置，未发送服务启停请求。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 构建与打包 | `pnpm tauri build --debug --no-bundle` 通过，随后以固定 `CARGO_TARGET_DIR=target/latest` 执行 `pnpm tauri bundle --debug --bundles app` 成功。新包约 45.20 MiB，包含最新中英界面和窗口尺寸 |
| 原生包完整性 | ad-hoc 签名、`codesign --verify --strict --deep`、`plutil -lint` 通过；包内二进制 SHA-256 为 `d286dd7961c6e70901da19600eba61908f5a946a10782557550ce57c6d20843f` |
| 原生窗口尺寸 | 从新包首次打开，未调整窗口。分离 Web Inspector 后，Tauri `inner_size` / `outer_size` 均为 2560×1600，`scale_factor=2`，实际逻辑尺寸为 1280×800；排除了面板停靠挤占 WebView 高度的影响 |
| 实际系统语言识别 | 系统 `AppleLanguages` 首项与新包 `system_language` 均为 `zh-Hans-CN`，首次检查 `devnook.language=null`、文档 lang 为 `zh-CN`。原生项目页、导航和语言设置均正确显示中文 |
| 双向切换与持久化 | 在设置语言菜单选择 English，导航、标题、说明、辅助标签即时更新；原生英文设置页截图检查通过。仅退出本轮启动的新版 GUI，再从同一 latest 包重新打开，项目页和设置仍为英文；随后通过设置切回简体中文，文案即时恢复。最终新版窗口保持打开，停留中文设置页 |
| 现有后台与服务 | 关闭新版 GUI 后及重新打开后，只读 Core 快照的服务运行 ID、READY 状态、配置版本、PID / PGID、runner 微秒启动身份、workspace_run_id、停止标记和历史记录保持一致。Core PID 65870、runner PID 95832、服务 PID 95836、实际监听子进程 PID 95880 及其启动时间 / 可执行路径未变；Core 核验的 DEVNOOK 监听仍为 `:5001` |
| 产物清理 | 新包验收后再次核对字面量路径、进程引用与 `lsof`，清理已无运行引用的 current / preview / browser 旧包、本轮旧 latest 回退副本与 latest 中间二进制，共约 223.70 MiB。保留最新 GUI 包及仍被 Core / runner 使用的 debug 包；未主动退出旧 Core、runner 或用户服务 |

原生 GUI 退出 / 重开不影响后台。本机系统语言没有修改；其他系统语言的默认选择沿用上一节单测与隔离页面验收结果，本轮未在非中文系统或旧 macOS / Intel 真机重测。原生面板已关闭，没有改动真实 MCP 配置或授权。只读前后快照与清理记录位于忽略目录 `.tmp/native-i18n-20261114/`。未提交、推送或发布。

## 中英文开源首页与 MIT 许可（2026-11-14）

- 默认 `README.md` 改为中文开源首页，新增信息对应的 `README.en.md`，页首提供双向语言切换。首页按产品用途、界面预览、功能、源码上手、CLI / MCP、数据边界、开发与贡献组织；不添加未发布的下载入口、远端 CI 成功徽章、用户指标或未实测的发布包体积。
- 原有详细操作、扫描限制、数据与升级说明迁至 `docs/USAGE.md`，更新相对链接，首页保留摘要与文档入口。清除首页中只适用于当前机器的临时工具链说明及重复的实现 / 验收叙述。
- 两版首页分别引用已检查的中文 / 英文 1280×800 端口界面预览，明确标注演示数据；图片从已有隔离验收产物复制到 `docs/images/readme-ports-zh.png` / `readme-ports-en.png`，不依赖临时目录，不包含用户真实项目和进程数据。
- 用户明确选择 MIT 后新增标准 `LICENSE`，版权署名为 `2026 Hresh`；保留第三方许可说明并在两版首页链接。没有修改业务代码、构建配置、依赖、权限或数据模型。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| Markdown 格式 | `pnpm exec prettier --check README.md README.en.md docs/USAGE.md` 通过 |
| 链接与结构 | 62 个本地链接 / 图片引用 / 页内锚点检查通过，代码围栏、段落与 details 标签配对；两张 PNG 均为 1280×800 |
| 中英一致性 | 5 组 Shell / JSON 代码块逐字一致；MCP JSON 可解析，默认仅含 `mcp serve`，没有控制参数或凭证。人工逐节核对功能、平台要求、语言默认规则、进程控制与已知限制，无能力差异 |
| 命令与版本依据 | `pnpm` 别名对照 `package.json`；Node.js 要求对照安装版 Vite 的 `^20.19.0 |  | >=22.12.0`；实际执行 CLI `--help`、`service --help`、`mcp serve --help`，确认命令入口与参数。未执行安装、服务查询 / 写入、启停或重新打包 |
| 使用指南迁移 | 去除旧首页的代码入口段落并修正链接后，迁移正文与原稿逐段相同；保护脚本的数字、代码、术语检查通过，人工复核扫描 / 执行 / 停止 / 升级条件保持 |
| MIT 正文 | 对照 Open Source Initiative 的 MIT 文本，并与本地依赖的标准 MIT 授权正文做空白归一化比较，正文一致；版权归属单独填写，不改第三方版权 |

仅修改文档、许可及首页图片。未重跑无关业务测试，未操作用户 GUI、Core、runner、数据库或正在运行的服务。GitHub 线上渲染、远端克隆和公开发布不在本次操作范围；文档内容依据本地源码、清单与已存在的验收记录核对。原稿与检查记录位于忽略目录 `.tmp/readme-20261114/`。未提交、推送或发布。

## Release DMG、体积优化与 GitHub Pages 官网（2026-11-14）

- 先以默认 Release profile 构建基线，再启用根 workspace 的 thin LTO、单 codegen unit 与符号裁剪；保留 Rust 默认速度优化和 panic unwind。业务源码、权限、UDS v10 与 SQLite v3 不变。
- 固定输出 `target/release/bundle/macos/DevNook.app`、`target/release/bundle/dmg/DevNook_0.1.0_aarch64.dmg` 与 `SHA256SUMS`。目标为 Apple Silicon / macOS 12+，本机使用 ad-hoc 签名；没有 Developer ID 签名或 Apple 公证，不将完整性校验等同于公证。
- 新增 `site/` 中文默认 / 英文独立路径官网：米白与深蓝视觉、产品截图、项目 / 服务 / 端口工作流、可选 MCP、FAQ 与获取入口。参考指定 localhost 仓库和网站的信息结构，文案、代码和自有演示图独立维护，无第三方字体、统计服务或新增 npm 依赖；官网不连接 Core 或执行命令。
- `pnpm site:build` 生成 `dist/site/`，`pnpm site:check` 校验静态链接与发布状态，`pnpm site:preview` 支持 `/devnook/` 项目路径。新增固定 commit 的 GitHub Pages workflow；DMG 通过 Releases 分发，站点清单仍为 `published: false`，未上传资产只显示源码构建入口。
- 同步中英文 README、MVP、架构、技术栈、项目边界、第三方说明和参考记录，新增 `docs/RELEASING.md` / `docs/GITHUB-PAGES.md`，记录构建、签名状态、部署与仓库 About / Topics 信息。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| Release 基线 / 优化构建 | 两次 `pnpm tauri build --no-bundle` 成功。默认主程序 17,892,208 字节，优化后打包前为 10,871,264 字节，减少约 39.2%；前端 TypeScript / Vite production build 同步通过 |
| 打包与实际体积 | `APPLE_SIGNING_IDENTITY=- CI=true pnpm tauri bundle --bundles app,dmg` 成功。最终 `.app` 为 10,841,548 字节（10.84 MB），DMG 为 4,432,224 字节（4.43 MB）；应用比原 47,137,596 字节 Debug 包减少约 77%。MB 使用十进制，Tauri 的 10.34 / 4.23 MiB 为同一产物 |
| 签名与 DMG | `codesign --verify --strict --deep`、`hdiutil verify` 通过；只读挂载后，包内主程序与 Info.plist 和构建产物逐字节一致，架构为 arm64，版本 0.1.0，最低系统 12.0。DMG 包含应用及 Applications 入口 |
| DMG 校验和 | SHA-256：`9b246e40a26fd8ee973f42caf31b10a8b112cd3fb44098499e923748e077b79a`；`scripts/record-release.mjs` 复算并登记体积与校验和，只记录未发布的 ad-hoc 候选 |
| Release 模式测试 | `cargo test --release --workspace --locked` 全量通过，包含进程身份、端口冲突、就绪、正常停止、依赖与回滚、CLI / MCP 授权等既有隔离测试。测试仅操作其临时数据与测试进程 |
| 原生包内启动 | 从只读 DMG 的应用实际打开，确认读取真实项目库、后台已连接及中文语言设置；截图观察通过。仅退出本轮测试 GUI 后卸载镜像，原 GUI、Core、runner 与服务保留 |
| 站点构建 / 发布状态 | `pnpm site:build` 和 `pnpm site:check` 通过；3 项发布逻辑测试覆盖未公开入口、正确资产链接、跨仓库 / 不完整 / 签名状态矛盾的清单拒绝。两种语言各有一份静态 HTML，34 个本地资源与锚点有效，输出 JS 语法通过 |
| 页面布局 | 中英 × 浅暗 × 320 / 375 / 414 / 768 / 1024 / 1440px 共 24 项通过，无页面横向溢出、导航越界或图片丢失。已修正英文 768px 顶栏按钮挤压；正文 16px、辅助文字 14px |
| 页面交互与截图 | 手机菜单 / Escape 焦点、锚点关闭菜单、双向语言路径、主题切换与重开记忆、浅暗应用图和大图链接、MCP JSON 复制 / 剪贴板失败降级、展开 FAQ、获取页体积 / 未发布状态、偏好存储失败降级及 reduced-motion 通过。桌面、375px 中英文、接入区与获取区截图已检查；HTTP 未知路径与越界路径为 404 |
| 对比度 / 运行错误 | 浏览器实际取样正文、次要文字、主按钮和深色接入区，浅暗最小值为 5.70:1；这是取样检查。最终页面未捕获运行错误或未处理 Promise。全局 Storage 失败替身曾干扰浏览器桥接，改为同步恢复 descriptor 的局部替身后通过，不修改浏览器全局配置 |
| 工作流与其他检查 | GitHub Pages Action 从官方仓库核对 commit，`actionlint` 通过；`pnpm check`、`pnpm test:ui`（11 项）与 `cargo fmt --all --check` 通过。新增源码和文档单独执行 Prettier 检查 |
| 既有服务保全 | 前后只读快照的配置版本、运行 ID / 历史、READY、PID / PGID、runner 微秒身份及停止标记一致；Core 65870、runner 95832、服务 95836 与实际监听子进程 95880 的身份未变，DEVNOOK 核验监听仍为 `:5001` |

官网预览通过 localhost MCP 新增独立 `site-preview` 服务（不参与 Start All），未修改或停止现有服务。该预览用于成品审阅。GitHub 远端目前为空仓库，尚未首次提交、推送、上传 Release、启用 Pages 或修改 About。公开发布需要独立授权；没有可用 Developer ID / 公证凭据，Intel、旧 macOS 实机、浏览器下载后的 Gatekeeper 行为及线上 Pages 尚未验收。Ego Lite 提示可更新，本轮未升级。临时验证记录位于 `.tmp/release-pages-20261114/`，官网资源与分发文档不依赖临时目录。

## 官网视觉升级（2026-11-14）

- 按用户要求调整本地官网：石墨 / 冷蓝 / 钴蓝 / 青色分层，重排首屏介绍与操作，加入静态本机架构示意，强化产品截图、工作流、MCP 与获取区域的视觉层次。默认暗色，已保存的浅暗选择仍优先；太阳 / 月亮图标与下一步切换动作对应。
- 修改限定在 `site/style.css`、`site/template.mjs`、`site/content.mjs`、`site/theme.js` 及设计记录。沿用自有产品截图与本机字体，无新增依赖、外部字体或图片请求；示意图仅用 HTML / CSS，不连接 Core、不伪造实时运行指标。
- 保留中文默认 / 英文独立路径、菜单、主题记忆、浅暗产品截图、原尺寸入口、MCP 复制、FAQ 和发布状态逻辑。`site/release.json` 及 DMG 未变，未发布的安装包继续显示源码构建入口。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 静态构建与检查 | `pnpm site:build`、`pnpm site:check` 通过；3 项发布元数据测试通过，双语言页面及 36 个本地资源 / 锚点有效，输出 JS 语法通过 |
| 响应式布局 | 中英 × 浅暗 × 320 / 375 / 414 / 768 / 1024 / 1280 / 1440px，共 28 项通过；无整页横向溢出、越界导航或图片缺失。桌面与手机截图已实际查看 |
| 交互 | 两种语言的手机菜单展开、Escape 关闭并回焦点、锚点导航关闭菜单、主题切换后重开记忆、产品截图切换和大图链接同步、MCP JSON 复制、FAQ 展开、未公开源码入口与体积信息均通过 |
| 主题启动 | 单独执行真实 `theme.js`，验证无手动偏好默认暗色、已存浅色 / 暗色优先、非法值与存储读取失败降级暗色，共 5 项通过 |
| 对比度及运行错误 | 两语言两主题分别取样正文、说明、主按钮、架构图、代码及获取面板，包含渐变背景的保守取样，最低 5.08:1；仅为取样检查。布局和交互路径未捕获 JS、未处理 Promise 或 CSP 错误 |
| 减少动态效果 | 实际模拟 reduced-motion，媒体查询命中、平滑滚动为 auto、按钮过渡为 0s |

复用现有 localhost `site-preview`（4310）供本地审阅，未启动或停止其他服务，未重打包或操作原生应用 / Core / runner。未修改 CI/CD、远端仓库或发布状态，未提交、推送或部署。浏览器截图与检查记录在忽略目录 `.tmp/site-premium-20261114/`；Safari / Firefox 和线上 Pages 未在本轮验收。Ego Lite 提示可更新，未升级。

## 官网石墨配色与产品主视觉（2026-11-14）

- 按用户明确确认的方向调整官网：石墨黑与白灰为主体，取消蓝色标题、背景光晕、网格和大面积蓝色区域；蓝色只保留在主操作、品牌细节和焦点。浅色版同步改为近白与灰色表面，工作流与 MCP 使用中性色。
- 真实应用截图进入首屏右侧并占更大宽度；原架构蓝图收成下方灰色关系说明带。手机先介绍后截图，平板采用双列文案加大图，桌面采用左右布局；保留双语路径、主题记忆、菜单、FAQ 和现有获取逻辑。
- 为统一当前 N 图标，使用现有 React 组件与隔离 fixture 重新采集中英 × 浅暗四张 1280×800 截图，替换 `site/assets/ports-*.png`。截图明确标注演示数据，不调用真实 Core、不展示个人项目；原图链接提供可见入口，与图片链接随主题预览同步。
- 修改范围为 `site/style.css`、`site/template.mjs`、`site/content.mjs`、`site/main.js`、四张站点截图及设计 / 开发记录。无新增依赖、外部字体或图像请求，不修改应用业务源码、发布清单或分发包。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 构建与静态检查 | `pnpm site:build`、`pnpm site:check` 通过；3 项发布状态测试通过，双语页面及 36 个本地链接 / 资源有效；站点修改源码的 Prettier 检查通过 |
| 布局与控件 | 中英 × 浅暗 × 320 / 375 / 414 / 768 / 1024 / 1280 / 1440px，共 28 项通过；1280px 使用 800px 高窗口。无页面横向溢出、图片缺失、可见导航越界或小于 44px 的被测按钮 / 语言 / 原图控件。桌面、手机及中英截图已实际查看 |
| 交互回归 | 中英文的菜单展开、Escape 回焦点、锚点关闭菜单、主题切换后重开记忆、浅暗预览与两个原图链接同步、MCP JSON 复制及失败选中降级、FAQ 展开、未发布源码入口、安装包体积展示均通过 |
| 图标与截图 | 真实 React 页面中采集四张 1280×800 PNG 并逐张查看，语言与浅暗正确、使用当前 N 图标、无运行错误；捕获的 invoke 仅限系统语言 / 主题及项目、设置、开发应用、端口和进程详情的只读 fixture 响应 |
| 可读性与动态偏好 | 两语言两主题取样正文、说明、主按钮正常 / hover、预览、关系说明、代码及发布面板，最低对比度 4.89:1；仅为取样检查。reduced-motion 模拟命中，平滑滚动为 auto、按钮过渡为 0s |
| 运行错误与状态保全 | 浏览器测试未捕获 JS、未处理 Promise 或 CSP 错误；CSS 变量引用均有定义。`site/release.json` 与本轮前逐字一致，仍为未公开的 ad-hoc Release，不出现虚假 DMG 下载入口 |

复用既有 `site-preview`（4310）供本地审阅。只为截图临时启动原本停止的 `web-ui`（1420），采集完成后核对本轮 PID 49146 并停止；现有官网预览及其他用户后台 / 服务未停止，未操作原生 GUI、Core、runner 或真实数据库。截图在独立 origin 和内存 invoke 替身中生成，无真实服务启停调用。未提交、推送、部署或公开发布，未修改 CI/CD、系统语言或真实 MCP 配置。Safari / Firefox 和线上 Pages 尚未实测。记录及回退副本位于 `.tmp/site-graphite-20261114/`。Ego Lite 提示可更新，未升级。

## 面向使用者的 README 与功能展示（2026-11-14）

- 根据当前使用说明和项目、扫描、开发应用、服务、端口、启动方案及高级接入的实际实现，建立 12 项功能覆盖清单；区分已有能力、必要条件、使用入口与截图证据，不把规划中的 IDEA 插件、远程管理或未发布下载写成现有功能。
- 更新中文默认 README 和英文 README：先交代 macOS 桌面产品形态，用找项目、开服务、查端口与组合环境四种任务说明用途，再按项目整理、服务日志、端口排查、依赖编排展开。补充外部服务正常停止 / 独立配置切换、日志历史 / 搜索、实际监听浏览器入口和 GUI 关闭后的后台行为；保留平台与扫描限制、确认与身份核验等边界。
- 官网沿用石墨黑 / 白灰和少量蓝色操作，首屏使用项目库实图及三项明确任务。移除 Core 架构说明带，增加导入预览、服务日志、端口详情、启动方案四段图文；桌面交错排列，手机先说明后图片。首次操作单独归纳，CLI / MCP 位于桌面用法之后，MCP 配置默认折叠。
- 自行采集五组真实 React 界面 × 中英 × 浅暗，共 20 张 1280×800 PNG。截图使用同一组隔离的演示工程、服务与日志，明确标注演示数据，不包含用户真实目录或进程。图片保存在 `site/assets/`，README 复用同一资源；全页图片与原图链接跟随主题，也可单独切换对比截图。
- 本轮修改仅涉及 README 两版、站点内容 / 模板 / 样式 / 前端交互 / 站点截图，以及设计与开发记录。未修改 Core 或应用业务逻辑、系统设置、真实数据库或 MCP 客户端配置；无新增依赖或外部媒体请求。

### 实际验证

| 检查 | 结果 |
| --- | --- |
| 构建与格式 | `pnpm site:build`、`pnpm site:check`、两版 README 与站点修改源码的 Prettier 检查通过；3 项发布状态测试通过，双语页面与 66 个本地资源 / 锚点有效，输出 JS 语法通过 |
| README 技术保全 | 两版各 7 个围栏代码块与原稿逐字相同，从“快速开始”起的技术正文经空白规范化后相同。中文技术部分保护脚本通过，并按原文核对平台、命令、权限、数据与进程边界。新增功能表述依据源码 / 说明逐项复核，不将截图模拟状态当作本轮真实服务执行证据 |
| README 链接与截图 | 本地链接、图片和页内锚点检查通过。20 张站点图片均为 1280×800；中文与英文代表性项目库、导入预览、服务日志、端口和启动方案图片已实际查看，图标与当前源文件一致 |
| 页面布局 | 中英 × 浅暗 × 320 / 375 / 414 / 768 / 1024 / 1280 / 1440px，共 28 项通过；图片加载完整，无整页横向溢出、越界导航或过小的被测交互控件。已查看桌面、手机和功能区的实际渲染 |
| 页面交互 | 菜单展开 / Escape 回焦点 / 锚点关闭、主题重开记忆、全部五组截图和各自原图链接同步、MCP 默认折叠 / 展开 / JSON 复制 / 剪贴板失败降级、FAQ、未发布源码入口通过。CLI 示例使用源码构建后的实际相对路径，不假定已安装到 PATH |
| 可读性与运行状态 | 取样正文、辅助文字、主按钮正常 / hover、功能说明、图注和终端示例，最低对比度 4.89:1，仅为取样检查。未捕获 JS、未处理 Promise 或 CSP 错误；所有 CSS 变量引用均有定义。reduced-motion 下平滑滚动为 auto，按钮过渡为 0s |

截图采集通过独立 origin 的内存 invoke 替身展示已有界面：查询返回演示数据，模拟扫描只返回预设候选，不执行真实扫描 / 导入 / 服务启停 / 应用打开。采集期间临时启动原本停止的 `web-ui`（1420，PID 57179），结束后核对身份并停止；既有 `site-preview`（4310）继续供本地审阅，原有运行后台和服务未停止。本轮没有执行打包、提交、推送、部署或公开发布；站点沿用当前未发布的 Release 清单。没有新增业务行为，因此未重跑无关 Rust 测试。Safari / Firefox、GitHub 线上渲染与 Pages 部署尚未实测。

功能覆盖、原稿副本、命令保护、链接检查与界面记录位于 `.tmp/product-story-20261114/`。已有应用图标 / 原生打包文档的独立更新予以保留，仅改官网设计段落。Ego Lite 提示可更新，未升级。
