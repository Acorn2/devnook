# Release 构建与分发

本文介绍 macOS Apple Silicon 与 Intel 的 Release 构建、校验与官网发布信息。公开分发的版本号、架构、资产名和 SHA-256 应与实际产物对应。

## 构建配置

根目录 `Cargo.toml` 的 Release profile 启用 thin LTO、单 codegen unit 和符号裁剪。沿用 Rust 默认的速度优化等级与 panic unwind，不改变 Core、进程控制、权限或数据协议。

首次构建前安装 README 中的开发依赖，并执行：

```sh
pnpm install --frozen-lockfile
pnpm tauri build --no-bundle
APPLE_SIGNING_IDENTITY=- CI=true pnpm tauri bundle --bundles app,dmg
node scripts/record-release.mjs
```

这里的 `APPLE_SIGNING_IDENTITY=-` 生成本机 ad-hoc 签名；`CI=true` 跳过依赖 Finder 交互的 DMG 布局步骤。镜像包含应用与 Applications 入口，支持拖入安装。

Apple Silicon 本机默认构建的固定输出：

```text
target/release/bundle/macos/DevNook.app
target/release/bundle/dmg/DevNook_0.1.4_aarch64.dmg
target/release/bundle/dmg/SHA256SUMS
```

`record-release.mjs` 核对 Mach-O 架构、应用版本、最低系统及包签名，计算实际体积与 SHA-256，并更新 `site/release.json` 中目标架构的 build。该脚本仅登记 ad-hoc 本地候选，设为 `published: false`、`url: null`，不会上传文件或创建 GitHub Release；同版本的另一架构保持原状，新版本不会继承旧版资产。版本号必须与根目录 `package.json`、Cargo workspace 和 Tauri 配置一致。

### Intel Mac 构建

在 macOS 上安装 x86_64 标准库，使用现有 Apple Clang / macOS SDK 交叉编译：

```sh
sh scripts/rust-local.sh rustup target add x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=12.0 pnpm tauri build --target x86_64-apple-darwin --no-bundle
APPLE_SIGNING_IDENTITY=- CI=true pnpm tauri bundle --target x86_64-apple-darwin --bundles app,dmg
node scripts/record-release.mjs --target x86_64-apple-darwin
```

固定输出为 `target/x86_64-apple-darwin/release/bundle/` 下的 `macos/DevNook.app`、`dmg/DevNook_0.1.4_x64.dmg` 与 `dmg/SHA256SUMS-x86_64`。Rust target / 清单架构使用 `x86_64`，DMG 文件名沿用 Tauri 的 `x64`。显式 `--target aarch64-apple-darwin` 同样使用独立 target 目录；不指定 target 则从本机架构和默认 `target/release/` 登记。

## 校验

```sh
sh scripts/rust-local.sh cargo test --release --workspace --locked
codesign --verify --strict --deep target/release/bundle/macos/DevNook.app
hdiutil verify target/release/bundle/dmg/DevNook_0.1.4_aarch64.dmg
cd target/release/bundle/dmg
shasum -a 256 -c SHA256SUMS
```

还应只读挂载 DMG，核对包内应用与构建产物一致，再检查实际启动与后台连接。验证时保留用户既有 Core、runner 与服务，不覆盖仍有进程引用的旧包。

Intel 测试使用 `cargo test --release --workspace --target x86_64-apple-darwin --locked`。Apple Silicon 上可通过已安装的 Rosetta 执行 x86_64 测试与应用，但不能据此声称 Intel 或旧 macOS 真机已验收。Release GUI 固定使用应用标识对应的数据目录，`DEVNOOK_DATA_DIR` 仅对 Debug GUI 生效；正式包的界面启动检查保持只读，隔离业务写入由 Core / CLI 测试承担。

## 签名与公证

**Release 编译模式与 Apple 公证是不同环节。** 当前本地构建使用 ad-hoc 签名，没有完成 Developer ID 签名或 Apple 公证。`codesign --verify` 验证包的签名完整性，不证明公证或 Gatekeeper 分发审核通过。

有 Developer ID Application 证书时，按 [Tauri macOS 签名与公证文档](https://v2.tauri.app/distribute/sign/macos/)配置构建环境并重新生成包；公证完成后再验证最终文件与校验和。不要把 Apple 凭据写入仓库。只有实际完成并验证后，才将官网的 `signing` 改为 `developer-id`、`notarized` 改为 `true`。

## 发布到 GitHub Releases

1. 确定版本号，并为对应源码创建版本标签，例如 `v0.1.4`。
2. 完成该版本的构建、测试和签名状态核对。
3. 上传相应架构的 DMG 与校验文件：Apple Silicon 使用 `DevNook_0.1.4_aarch64.dmg` / `SHA256SUMS`，Intel 使用 `DevNook_0.1.4_x64.dmg` / `SHA256SUMS-x86_64`。发布说明如实写明目标架构、系统要求和公证状态；同源版本追加新架构时保留既有资产，业务代码变化需使用新版本。
4. 从公开资产重新核对文件大小及 SHA-256；确认可下载后，再更新官网发布清单。

`site/release.json` 包含共同 `version` 和逐架构 `builds`。公开下载字段应合并到对应架构的 build，例如 aarch64：

```json
{
  "published": true,
  "url": "https://github.com/Acorn2/devnook/releases/download/v0.1.4/DevNook_0.1.4_aarch64.dmg"
}
```

这是待合并到单个 build 的字段片段，其余架构、体积、校验和与签名状态保留实际值。生成器只接受当前仓库、对应版本和对应架构的准确文件名，拒绝重复架构、链接串架构和矛盾状态。未公开构建只显示源码入口，不能遮蔽同版本另一架构的有效下载。

## 历史体积基线与当前产物

初始 Release 体积基线：2026-11-14，macOS Apple Silicon，版本 0.1.0；单位为十进制 MB：

| 产物                              |     大小 |
| --------------------------------- | -------: |
| 之前的 Debug `.app`               | 47.14 MB |
| 默认 Release 主程序               | 17.89 MB |
| 优化后的 Release 主程序（打包前） | 10.87 MB |
| 初始 Release `.app`               | 10.84 MB |
| 初始 Release DMG                  |  4.43 MB |

该次链接优化与符号裁剪使 Release 主程序比默认 Release 基线减少约 39.2%，初始 Release 应用比原 Debug 包减少约 77%。这些是当时产物的历史比较，不作为后续包的固定体积承诺。

同日确认内折角 N 图标后重新构建的 v0.1.0：`.app` 为 14,911,803 字节（14.91 MB），DMG 为 4,695,106 字节（4.70 MB），目标为 Apple Silicon / macOS 12+，采用 ad-hoc 签名，尚未公证。该产物已通过 [GitHub Release v0.1.0](https://github.com/Acorn2/devnook/releases/tag/v0.1.0) 公开分发，DMG 与 `SHA256SUMS` 均已从公开下载地址重新校验。版本标签对应源码 `4225ba51f0f91274f9a62e863c0aac40a74e5bb0`，中英文发布说明保存在 [v0.1.0.md](releases/v0.1.0.md)。

完整尺寸 ICNS 与 PNG 从同一 SVG 导出，macOS 打包配置优先使用 ICNS。实际体积、校验和与公开状态已更新至 `site/release.json`；DMG 与安装后的应用采用不同体积口径，未来版本以重新构建结果为准。

同源 Intel 构建已追加至 v0.1.0：`.app` 15,962,779 字节（15.96 MB），DMG 5,182,582 字节（5.18 MB），x86_64 / macOS 12+ / ad-hoc / 未公证。其独立校验文件为 `SHA256SUMS-x86_64`，原 Apple Silicon 包与校验文件不变。

官网部署见 [GitHub Pages](GITHUB-PAGES.md)。


## v0.1.1 双架构发行

2026-11-15 发布 [v0.1.1](https://github.com/Acorn2/devnook/releases/tag/v0.1.1)，标签源码为 `00cea9a58b9eb9bd15bd927c08b932300c210a10`。本次包含截至 f1234bc 的服务托管、AI 交接和网页入口改进；发布说明见 [v0.1.1.md](releases/v0.1.1.md)。

| 架构 | 应用字节 | DMG 字节 | SHA-256 |
| --- | ---: | ---: | --- |
| Apple Silicon | 14,978,795 | 4,730,761 | `7b9ca4b2183571a9841a64cc131129167b251cc814534e394ec6ff8d055b6764` |
| Intel Mac | 16,034,251 | 5,221,961 | `76394a2e1d27f361f1c9241f525cc6f01b6a2d4ff6b48d959116e10742a3b07e` |

两个应用的 Mach-O / Info.plist 均以 macOS 12.0 为最低版本，采用 ad-hoc 签名、未公证。两份 DMG 与对应校验文件均从公开地址重新下载核验，v0.1.0 Release 与四个原始资产保持不变。官网按架构登记 v0.1.1 的真实 URL / 体积 / 校验和；不覆盖旧版资产或将 Debug 包作为 Release 分发。

Release workspace 在 ARM 与 x86_64 / Rosetta 上各 92 项通过、5 个既有内部 fixture 忽略。Intel 完整矩阵使用 Apple Python 3.9.6；本机 Python 3.14 的四秒期限测试曾在整套执行中超时、单项通过，未改动测试或产品期限，详情保留在开发记录及发布说明。原生双架构窗口启动与连接检查保持只读，没有更新既有 Core、runner 或重启真实服务。Intel 与 macOS 12 真机仍未验收。

## v0.1.2 菜单栏发行

本版源码标签为 `v0.1.2`，对应提交 `1b650f812cf66df8e52603d9d413ab11a789034f`。菜单栏功能、安装说明与完整校验值见 [v0.1.2.md](releases/v0.1.2.md)。Apple Silicon 与 Intel 均为 macOS 12+、ad-hoc 签名、未公证；公开文件已重新下载核对。

| 架构 | 应用字节 | DMG 字节 | SHA-256 |
| --- | ---: | ---: | --- |
| Apple Silicon | 15,327,099 | 4,874,171 | `f8f51cc8a215a61a9f99697c34b5ffe6d9b769fbec17a4122df5393e05b35e2b` |
| Intel Mac | 16,446,731 | 5,386,514 | `1442dea91316b1d9d18a6c1572a97311e657c501f6d592408128868645450d38` |

旧版 v0.1.0 / v0.1.1 的资产与校验文件保持原样。两架构各 97 项 Release workspace 测试通过、5 个 fixture 忽略，38 项界面测试通过。原生菜单 / 多屏 / 全屏 Space、Intel 硬件及 macOS 12 真机的未验收范围见发布说明。

## v0.1.3 颜色标签发行

本版源码标签为 `v0.1.3`，对应提交 `383acde05b0df1e85458a28f5acc8fcfcb8ee89a`。项目 / 服务 / 实际端口的稳定配色、技术栈标签与官网更新见 [v0.1.3.md](releases/v0.1.3.md)。两个包均为 macOS 12+、ad-hoc 签名、未公证，公开文件已重新下载核验。

| 架构 | 应用字节 | DMG 字节 | SHA-256 |
| --- | ---: | ---: | --- |
| Apple Silicon | 15,327,083 | 4,874,808 | `04f7bdd07a59524a180dba5c1ad6c270d62f994f765247fddcde14edb04dabff` |
| Intel Mac | 16,446,763 | 5,387,570 | `9a89f057f7ba1c64246993db4078cfd76eef628b600734297369a4a69a982be6` |

旧版 v0.1.0–v0.1.2 的 Release 与资产保持原样。两架构各 97 项 Release workspace 测试通过，5 个 fixture 忽略，38 项前端测试通过。原生窗口 / 菜单交互、Intel 硬件及 macOS 12 真机本轮未补测。
