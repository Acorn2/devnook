# Release 构建与分发

本文介绍 macOS Apple Silicon 的本地 Release 构建、校验与官网发布信息。公开分发的版本号、资产名和 SHA-256 应与实际产物对应。

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

固定输出：

```text
target/release/bundle/macos/DevNook.app
target/release/bundle/dmg/DevNook_0.1.0_aarch64.dmg
target/release/bundle/dmg/SHA256SUMS
```

`record-release.mjs` 核对包签名、计算实际体积及 SHA-256，并将结果写入 `site/release.json`。该脚本仅登记 ad-hoc 本地候选，设为 `published: false`、`url: null`，不会上传文件或创建 GitHub Release。版本号必须与根目录 `package.json`、Cargo workspace 和 Tauri 配置一致。

## 校验

```sh
sh scripts/rust-local.sh cargo test --release --workspace --locked
codesign --verify --strict --deep target/release/bundle/macos/DevNook.app
hdiutil verify target/release/bundle/dmg/DevNook_0.1.0_aarch64.dmg
cd target/release/bundle/dmg
shasum -a 256 -c SHA256SUMS
```

还应只读挂载 DMG，核对包内应用与构建产物一致，再检查实际启动与后台连接。验证时保留用户既有 Core、runner 与服务，不覆盖仍有进程引用的旧包。

## 签名与公证

**Release 编译模式与 Apple 公证是不同环节。** 当前本地构建使用 ad-hoc 签名，没有完成 Developer ID 签名或 Apple 公证。`codesign --verify` 验证包的签名完整性，不证明公证或 Gatekeeper 分发审核通过。

有 Developer ID Application 证书时，按 [Tauri macOS 签名与公证文档](https://v2.tauri.app/distribute/sign/macos/)配置构建环境并重新生成包；公证完成后再验证最终文件与校验和。不要把 Apple 凭据写入仓库。只有实际完成并验证后，才将官网的 `signing` 改为 `developer-id`、`notarized` 改为 `true`。

## 发布到 GitHub Releases

1. 确定版本号，并为对应源码创建版本标签，例如 `v0.1.0`。
2. 完成该版本的构建、测试和签名状态核对。
3. 将 `DevNook_0.1.0_aarch64.dmg` 与 `SHA256SUMS` 上传到该版本的 GitHub Release，发布说明如实写明目标架构、系统要求和公证状态。
4. 从公开资产重新核对文件大小及 SHA-256；确认可下载后，再更新官网发布清单。

`site/release.json` 的公开下载字段示例：

```json
{
  "published": true,
  "url": "https://github.com/Acorn2/devnook/releases/download/v0.1.0/DevNook_0.1.0_aarch64.dmg"
}
```

这是待合并的字段片段，其余体积、校验和与签名状态保留实际值。生成器只接受当前仓库、对应版本和准确文件名的下载地址；清单不完整或状态矛盾会阻止站点构建。未公开时，官网显示源码构建入口，不显示无效的 DMG 下载按钮。

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

同日确认内折角 N 图标后重新构建的当前本地候选：`.app` 为 14,911,803 字节（14.91 MB），DMG 为 4,695,106 字节（4.70 MB），仍为版本 0.1.0 / Apple Silicon / ad-hoc，尚未公证或公开发布。完整尺寸 ICNS 与 PNG 从同一 SVG 导出，macOS 打包配置优先使用 ICNS。实际体积、校验和与发布状态已更新至 `site/release.json`；DMG 与安装后的应用采用不同体积口径，未来版本以重新构建结果为准。

官网部署见 [GitHub Pages](GITHUB-PAGES.md)。
