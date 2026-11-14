# DevNook GitHub Pages

官网源码位于 `site/`，生成结果位于 `dist/site/`。采用纯静态 HTML、CSS 与 JavaScript，不需要服务端、数据库或新增 npm 依赖。

- 中文默认路径：`/devnook/`
- 英文路径：`/devnook/en/`
- 预期公开地址：`https://acorn2.github.io/devnook/`

此地址只有在仓库内容推送、Pages 设置和部署完成后才可用。本地构建不会自动发布。

## 本地构建与检查

```sh
pnpm site:build
pnpm site:check
pnpm site:preview
```

预览入口为 `http://127.0.0.1:4310/devnook/`，`PORT` 可更改监听端口。预览服务器只监听 loopback，并限制在站点目录下；本地开发服务由 localhost MCP 管理时，启动项目中的 `site-preview` 服务即可。

## 文件入口

| 文件                             | 用途                                         |
| -------------------------------- | -------------------------------------------- |
| `site/content.mjs`               | 中英文文案、仓库地址和 Pages 路径            |
| `site/template.mjs`              | 静态 HTML 生成与发布清单校验                 |
| `site/style.css`                 | 响应式布局与浅暗主题                         |
| `site/main.js` / `site/theme.js` | 导航、主题、截图与复制交互                   |
| `site/assets/`                   | 本项目 Logo 和使用演示数据的应用截图         |
| `site/release.json`              | 实际版本、架构、体积、校验和、签名和公开状态 |
| `scripts/build-site.mjs`         | 生成两种语言的 HTML、静态资源与 sitemap      |
| `.github/workflows/pages.yml`    | 静态构建、检查及 GitHub Pages 部署           |

截图来自本项目的隔离界面验收，不包含用户真实项目或进程。官网展示只读 MCP 配置样例，不连接本机 Core、扫描目录或执行服务命令。主题偏好独立保存在 `devnook.site.appearance`；不加载第三方字体或统计脚本。

## GitHub Pages 设置

1. 将审阅后的项目源码提交并推送至 `Acorn2/devnook` 的 `main` 分支。
2. 在仓库 **Settings → Pages → Build and deployment** 中，将 **Source** 设为 **GitHub Actions**。
3. 确认需要公开官网后，在 Actions 中手动运行 **GitHub Pages** workflow；推送源码不会触发部署。
4. 确认 build 与 deploy 均成功，再访问公开地址，并检查英文路径、图片、主题和下载入口。

工作流仅向 Pages 上传 `dist/site/`，不上传整个仓库、真实应用数据或 DMG。build job 只读仓库与 Pages 配置；deploy job 使用 `pages: write` 和 `id-token: write`，部署环境为 `github-pages`。Action 引用固定为从官方仓库核对的 commit。

常规 CI 会在源码推送或 Pull Request 时构建并检查官网，不调用 Pages 部署。发布工作流仅保留 `workflow_dispatch`，避免源码更新隐式公开官网。

如果 fork 到其他仓库，请修改 `site/content.mjs` 的 `repo`、`origin` 和 `basePath`，同时同步发布资产 URL 的校验规则。保持官网路径与预览服务器一致。

配置依据：[GitHub Pages 自定义工作流](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)。

## 下载状态

本地 DMG 的存在不代表已公开发布。`site/release.json` 默认 `published: false`，展示测得的体积与签名状态，并提供源码构建入口。

DMG 应存放在 GitHub Releases。完成上传并核对公开文件后，按 [Release 分发说明](RELEASING.md)填写准确 URL，将 `published` 改为 `true`，再部署官网。不要仅为显示下载按钮而更改状态，也不要把 ad-hoc 签名写成已通过 Apple 公证。

## 仓库 About 信息

网站部署完成后，可将 GitHub 仓库侧栏的 Website 设置为已验证可访问的 Pages 地址。Description 建议：

> Local development workspace for macOS. Discover projects, manage dev services and logs, inspect ports, and share the same workspace with CLI and MCP. Built with Tauri and Rust.

Topics 可按实际能力填写：

```text
desktop-app developer-tools local-development macos dev-server
process-manager mcp mcp-server rust tauri react typescript
```

这些字段需要在仓库设置中单独保存；本地文件不会自动修改远端 About、Topics 或 Pages 设置。
