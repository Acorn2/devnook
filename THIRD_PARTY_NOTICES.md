# Third-party notices

DevNook does not incorporate source code or visual assets from CodeDeck, Runbox or localhost. Their designs were studied at the commits recorded in `docs/REFERENCE-RESEARCH.md`. All three inspected LICENSE files identify MIT licensing; any future code reuse must preserve the corresponding notices and record the exact files and changes here.

Dependency versions are pinned by `Cargo.lock` and `pnpm-lock.yaml`. Dependencies retain their own licenses, including React (MIT), Vite (MIT), TypeScript (Apache-2.0), Tauri (MIT OR Apache-2.0), rusqlite (MIT), serde (MIT OR Apache-2.0), uuid (MIT OR Apache-2.0), libc (MIT OR Apache-2.0), and Lucide icons (ISC). SQLite is public domain.

The DevNook N logo is authored in this repository at `apps/desktop/public/devnook.svg`; it is not copied from a reference product.

Phase 2 adds security-framework and url (both MIT OR Apache-2.0), pinned in Cargo.lock. No reference-project source or assets were copied for service supervision.

Phase 3 adds clap (MIT OR Apache-2.0) and sha2 (MIT OR Apache-2.0), pinned in Cargo.lock. The stdio MCP adapter follows the public protocol specification; no third-party server implementation is copied.

The desktop language adapter directly uses the already pinned objc2-foundation 0.3.2 (MIT) to read the first macOS preferred language. No system language setting is changed.

The DevNook product website uses original HTML, CSS, copy, the repository's own logo, and screenshots captured from its isolated UI fixtures. The localhost website and repository informed the information structure; their source code, copy, screenshots, and visual assets are not incorporated. The website uses locally available system fonts and no third-party font service.
