//! Project services need entry-point evidence, not just a project cwd. Tool parents
//! never taint their children; every listening PID is classified independently.
use crate::{model::ProcessKind, service_drafts::parse_command};
use std::path::{Component, Path, PathBuf};

fn normalized(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            _ => result.push(part),
        }
    }
    result.is_absolute().then_some(result)
}

fn owned(entry: &str, cwd: Option<&str>, roots: &[&Path]) -> bool {
    let path = Path::new(entry);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(cwd) = cwd {
        Path::new(cwd).join(path)
    } else {
        return false;
    };
    let Some(path) = normalized(&path) else {
        return false;
    };
    roots.iter().any(|root| {
        path.strip_prefix(root).is_ok_and(|relative| {
            // An arbitrary installed CLI is not a project entry point.
            !relative.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                matches!(s.as_ref(), "node_modules" | ".venv" | "venv" | ".git")
                    || s.ends_with(".app")
            })
        })
    })
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
}

fn tool_entry(entry: &str) -> bool {
    let entry = entry.replace('\\', "/").to_lowercase();
    [
        "/@openai/codex/",
        "/@anthropic-ai/claude-code/",
        "/@google/gemini-cli/",
        "/typescript/lib/tsserver",
        "/typescript-language-server/",
        "/pyright/",
        "/basedpyright/",
        "/vscode-",
        "/.vscode/extensions/",
        "/.cursor/extensions/",
        "/resources/app/out/",
        "/resources/app/extensions/",
    ]
    .iter()
    .any(|marker| entry.contains(marker))
        || matches!(
            entry.as_str(),
            "aider" | "aider.main" | "debugpy" | "pyright" | "ipykernel_launcher"
        )
}

fn tool_executable(executable: &str) -> bool {
    let name = basename(executable).to_lowercase();
    if matches!(
        name.as_str(),
        "codex"
            | "workbuddy"
            | "claude"
            | "cursor-agent"
            | "opencode"
            | "gemini"
            | "aider"
            | "code"
            | "cursor"
            | "windsurf"
            | "qoder"
            | "trae"
            | "zed"
            | "idea"
            | "webstorm"
            | "pycharm"
            | "goland"
            | "rustrover"
            | "rider"
            | "clion"
            | "xcode"
            | "sourcekit-lsp"
            | "rust-analyzer"
            | "gopls"
    ) {
        return true;
    }
    let path = executable.to_lowercase();
    [
        "/visual studio code.app/",
        "/cursor.app/",
        "/windsurf.app/",
        "/workbuddy.app/",
        "/qoder.app/",
        "/trae.app/",
        "/intellij idea",
        "/webstorm.app/",
        "/pycharm",
        "/goland.app/",
        "/rustrover.app/",
        "/rider.app/",
        "/clion.app/",
        "/xcode.app/",
    ]
    .iter()
    .any(|marker| path.contains(marker))
}

fn server_entry(entry: &str) -> bool {
    let path = entry.replace('\\', "/");
    // Match the launched package entry, never arguments mentioning these names.
    [
        "/vite/bin/vite",
        "/next/dist/bin/next",
        "/next/dist/server/lib/start-server",
        "/nuxt/bin/",
        "/nuxi/bin/",
        "/astro/astro.js",
        "/webpack-dev-server/bin/",
        "/@expo/cli/build/bin/",
        "/expo/bin/cli",
        "/react-native/cli.js",
        "/metro/src/cli",
    ]
    .iter()
    .any(|marker| path.contains(marker))
}

fn script_entry<'a>(args: &'a [String], value_options: &[&str]) -> Option<&'a str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            return iter.next().map(String::as_str);
        }
        if matches!(arg.as_str(), "-e" | "--eval" | "-p" | "--print" | "-c") {
            return None;
        }
        if value_options.contains(&arg.as_str()) {
            iter.next()?;
        } else if arg.starts_with('-') {
            // Unknown value-taking switches cannot safely identify the entry.
            if !arg.contains('=')
                && !matches!(
                    arg.as_str(),
                    "--watch" | "--no-warnings" | "--enable-source-maps" | "-u" | "-B" | "-O"
                )
                && !arg.starts_with("--inspect")
            {
                return None;
            }
        } else {
            return Some(arg);
        }
    }
    None
}

/// Positive evidence for promoting a browser shortcut, not proof that `/`
/// returns HTML. Generic runtimes, API servers and mobile bundlers stay unknown.
pub(crate) fn browser_entry(executable: Option<&str>, command: &str) -> bool {
    let Some(executable) = executable else {
        return false;
    };
    let name = basename(executable).to_lowercase();
    if !matches!(name.as_str(), "node" | "nodejs" | "bun" | "deno") {
        return false;
    }
    if name == "node"
        && command.strip_prefix("next-server (v").is_some_and(|v| {
            v.strip_suffix(')').is_some_and(|v| {
                !v.is_empty()
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
            })
        })
    {
        return true;
    }
    let line = command
        .strip_prefix(executable)
        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace));
    let parsed = line
        .map(|rest| parse_command(&format!("runtime {rest}")))
        .unwrap_or_else(|| parse_command(command));
    let Ok(spec) = parsed else { return false };
    let args = if spec.args.first().is_some_and(|a| a == "run") {
        &spec.args[1..]
    } else {
        &spec.args[..]
    };
    let Some(entry) = script_entry(
        args,
        &[
            "-r",
            "--require",
            "--import",
            "--loader",
            "--experimental-loader",
            "--conditions",
        ],
    ) else {
        return false;
    };
    // Match the actual package entry, never a project dependency, directory name,
    // another argument mentioning a frontend tool, or an arbitrary server.js.
    [
        "/vite/bin/vite.js",
        "/next/dist/bin/next",
        "/next/dist/server/lib/start-server.js",
        "/astro/astro.js",
        "/nuxi/bin/nuxi.mjs",
        "/nuxt/bin/nuxt.mjs",
        "/webpack-dev-server/bin/webpack-dev-server.js",
    ]
    .iter()
    .any(|suffix| entry.ends_with(suffix))
}

pub(crate) fn classify(
    executable: Option<&str>,
    command: &str,
    cwd: Option<&str>,
    roots: &[&Path],
) -> ProcessKind {
    use ProcessKind::*;
    let Some(executable) = executable else {
        return Unknown;
    };
    let name = basename(executable).to_lowercase();
    // Native AI tools may accept arbitrary commands in their prompt/arguments.
    if matches!(
        name.as_str(),
        "codex" | "workbuddy" | "claude" | "cursor-agent" | "opencode" | "gemini" | "aider"
    ) {
        return DevelopmentTool;
    }
    // Next replaces argv with a process title; its parent is irrelevant.
    if name == "node"
        && command.strip_prefix("next-server (v").is_some_and(|v| {
            v.strip_suffix(')').is_some_and(|v| {
                !v.is_empty()
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
            })
        })
        && cwd.is_some_and(|cwd| owned(cwd, None, roots))
    {
        return Service;
    }
    if name.starts_with("python")
        && ["gunicorn: master [", "gunicorn: worker ["]
            .iter()
            .any(|prefix| {
                command
                    .strip_prefix(prefix)
                    .is_some_and(|rest| !rest.is_empty() && rest.ends_with(']'))
            })
        && cwd.is_some_and(|cwd| owned(cwd, None, roots))
    {
        return Service;
    }
    // ps does not retain argv boundaries. Strip the kernel executable first so
    // paths such as IDEA's bundled JBR can contain spaces. Ambiguity stays unknown.
    let line = command
        .strip_prefix(executable)
        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace));
    let parsed = line
        .map(|rest| parse_command(&format!("runtime {rest}")))
        .unwrap_or_else(|| parse_command(command));
    if let Ok(spec) = parsed {
        let args = &spec.args;
        let node = matches!(name.as_str(), "node" | "nodejs" | "bun" | "deno")
            || name.starts_with("code helper");
        if node {
            let args = if args.first().is_some_and(|a| a == "run") {
                &args[1..]
            } else {
                args.as_slice()
            };
            if let Some(entry) = script_entry(
                args,
                &[
                    "-r",
                    "--require",
                    "--import",
                    "--loader",
                    "--experimental-loader",
                    "--conditions",
                ],
            ) {
                if tool_entry(entry) {
                    return DevelopmentTool;
                }
                if server_entry(entry)
                    || (owned(entry, cwd, roots)
                        && ["js", "mjs", "cjs", "ts", "mts", "cts"]
                            .iter()
                            .any(|ext| entry.ends_with(&format!(".{ext}"))))
                {
                    return Service;
                }
            }
        } else if name.starts_with("python") || name.starts_with("pypy") {
            if let Some(index) = args.iter().position(|a| a == "-m") {
                // Only interpreter switches may precede -m, not an earlier script.
                if args[..index]
                    .iter()
                    .all(|a| ["-u", "-B", "-O"].contains(&a.as_str()))
                {
                    if let Some(module) = args.get(index + 1) {
                        if tool_entry(module) {
                            return DevelopmentTool;
                        }
                        if ["uvicorn", "gunicorn", "hypercorn", "daphne", "http.server"]
                            .contains(&module.as_str())
                            || (module == "flask" && args[index + 2..].iter().any(|a| a == "run"))
                        {
                            return Service;
                        }
                    }
                }
            } else if let Some(entry) = script_entry(args, &[]) {
                if tool_entry(entry) {
                    return DevelopmentTool;
                }
                if (owned(entry, cwd, roots) && entry.ends_with(".py"))
                    || ["uvicorn", "gunicorn", "hypercorn", "daphne"].contains(&basename(entry))
                {
                    return Service;
                }
            }
        } else if name == "java" {
            let mut iter = args.iter();
            while let Some(arg) = iter.next() {
                if [
                    "-cp",
                    "-classpath",
                    "--class-path",
                    "-p",
                    "--module-path",
                    "--add-opens",
                    "--add-exports",
                    "--add-reads",
                    "--add-modules",
                    "--enable-native-access",
                    "--patch-module",
                    "--limit-modules",
                ]
                .contains(&arg.as_str())
                {
                    let _ = iter.next();
                    continue;
                }
                if arg == "-jar" {
                    if iter
                        .next()
                        .is_some_and(|jar| jar.ends_with(".jar") && owned(jar, cwd, roots))
                    {
                        return Service;
                    }
                    break;
                }
                if arg.starts_with('-') {
                    continue;
                }
                if [
                    "com.intellij.",
                    "org.jetbrains.",
                    "org.gradle.",
                    "org.apache.maven.",
                    "org.codehaus.plexus.classworlds.",
                ]
                .iter()
                .any(|p| arg.starts_with(p))
                {
                    return DevelopmentTool;
                }
                if arg.starts_with("org.springframework.boot.loader.")
                    || args.windows(2).any(|pair| {
                        ["-cp", "-classpath", "--class-path"].contains(&pair[0].as_str())
                            && pair[1]
                                .split(':')
                                .any(|p| owned(p.trim_end_matches('*'), cwd, roots))
                    })
                {
                    return Service;
                }
                break;
            }
        } else if name == "dotnet" {
            if args
                .first()
                .is_some_and(|a| a.ends_with(".dll") && owned(a, cwd, roots))
            {
                return Service;
            }
        } else if name.starts_with("php") {
            if args.first().is_some_and(|a| a == "-S")
                || args.first().is_some_and(|a| {
                    basename(a) == "artisan" && args.get(1).is_some_and(|a| a == "serve")
                })
            {
                return Service;
            }
        } else if name == "ruby" {
            if args
                .first()
                .is_some_and(|a| ["rails", "puma", "rackup"].contains(&basename(a)))
                || args
                    .first()
                    .is_some_and(|a| a.ends_with(".rb") && owned(a, cwd, roots))
            {
                return Service;
            }
        } else if name == "beam.smp" {
            // BEAM can also host ElixirLS; a project cwd alone is insufficient.
            if args
                .iter()
                .any(|a| a.contains("/elixir-ls/") || a.contains("/.elixir_ls/"))
            {
                return DevelopmentTool;
            }
            if args
                .windows(2)
                .any(|pair| basename(&pair[0]) == "mix" && pair[1] == "phx.server")
                || args
                    .windows(2)
                    .any(|pair| pair[0] == "-boot" && owned(&pair[1], cwd, roots))
            {
                return Service;
            }
        }
    }
    // Bundled runtimes can run real services: this fallback follows entry checks.
    if tool_executable(executable) {
        return DevelopmentTool;
    }
    if !matches!(
        name.as_str(),
        "node" | "nodejs" | "bun" | "deno" | "java" | "dotnet" | "ruby"
    ) && !name.starts_with("python")
        && !name.starts_with("pypy")
        && !name.starts_with("php")
        && (owned(executable, None, roots)
            || (executable.contains("/go-build")
                && executable.contains("/exe/")
                && cwd.is_some_and(|cwd| owned(cwd, None, roots))))
    {
        return Service;
    }
    Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kind(exe: &str, command: &str) -> ProcessKind {
        classify(
            Some(exe),
            command,
            Some("/work/app"),
            &[Path::new("/work/app")],
        )
    }

    #[test]
    fn browser_shortcuts_need_an_actual_web_tool_entry_not_just_a_node_service() {
        for command in [
            "node /work/app/node_modules/vite/bin/vite.js",
            "node --require hook.js /work/app/node_modules/vite/bin/vite.js",
            "node /work/app/node_modules/next/dist/bin/next dev",
            "node /work/app/node_modules/astro/astro.js dev",
            "next-server (v16.0.1)",
        ] {
            assert!(browser_entry(Some("/bin/node"), command), "{command}");
        }
        for command in [
            "node /work/app/local-server/server.mjs",
            "node /work/app/backend/server.js",
            "node /work/app/node_modules/@nestjs/cli/bin/nest.js start",
            "node server.js --config /work/app/node_modules/vite/bin/vite.js",
            "node -e 'require(\"vite\")' /work/app/node_modules/vite/bin/vite.js",
            "node /work/app/node_modules/vite/bin/vite.js.backup",
            "node /work/app/node_modules/@expo/cli/build/bin/cli",
            "node /work/app/node_modules/react-native/cli.js",
            "node /work/app/node_modules/metro/src/cli.js",
            "next-server (v16.0.1) extra-arguments",
        ] {
            assert!(!browser_entry(Some("/bin/node"), command), "{command}");
        }
        assert!(!browser_entry(
            None,
            "node /work/app/node_modules/vite/bin/vite.js"
        ));
        assert!(!browser_entry(Some("/bin/java"), "java -jar backend.jar"));
    }

    #[test]
    fn tools_are_not_services_and_arguments_do_not_supply_entry_evidence() {
        for (exe, command) in [
            ("/bin/codex", "codex --prompt 'node server.js'"),
            (
                "/bin/node",
                "node /global/node_modules/@openai/codex/bin/codex.js",
            ),
            (
                "/bin/node",
                "node /global/node_modules/typescript/lib/tsserver.js",
            ),
            ("/bin/python3", "python3 -m aider --file server.py"),
            (
                "/Applications/IntelliJ IDEA.app/Contents/jbr/bin/java",
                "java -cp /work/app/build com.intellij.idea.Main",
            ),
            (
                "/bin/java",
                "java -cp /work/app/build org.gradle.launcher.daemon.bootstrap.GradleDaemon",
            ),
        ] {
            assert_eq!(
                kind(exe, command),
                ProcessKind::DevelopmentTool,
                "{command}"
            );
        }
        for (exe, command) in [
            ("/bin/node", "node -e 'listen()' --filename server.js"),
            ("/bin/node", "node /global/tool.js /work/app/server.js"),
            ("/bin/node", "node ../../outside/server.js"),
            ("/bin/python3", "python3 -c 'listen()' server.py"),
            ("/bin/python3", "python3 /global/tool.py --module uvicorn"),
            ("/bin/java", "java -Dspring=boot UnknownTool"),
            ("/usr/bin/unrecognized", "unrecognized --cwd /work/app"),
        ] {
            assert_eq!(kind(exe, command), ProcessKind::Unknown, "{command}");
        }
    }

    #[test]
    fn services_keep_their_identity_regardless_of_ai_or_ide_parent() {
        for (exe, command) in [
            ("/bin/node", "node server.js"),
            ("/bin/node", "node --require hook.js codex-demo/server.js"),
            ("/bin/node", "node /global/node_modules/vite/bin/vite.js"),
            ("/bin/node", "next-server (v16.0.1)"),
            ("/bin/node", "node /global/node_modules/@expo/cli/build/bin/cli"),
            ("/bin/python3", "python3 -u manage.py runserver"),
            ("/bin/python3", "python3 -m uvicorn app:app"),
            ("/bin/python3", "gunicorn: worker [app:app]"),
            ("/bin/java", "java -jar target/backend.jar"),
            ("/bin/java", "java --add-opens java.base/java.lang=ALL-UNNAMED -cp target/classes com.example.App"),
            ("/bin/beam.smp", "beam.smp -- -noshell -extra /bin/mix phx.server"),
            ("/Applications/IntelliJ IDEA.app/Contents/jbr/bin/java", "/Applications/IntelliJ IDEA.app/Contents/jbr/bin/java -cp /work/app/target/classes com.example.Application"),
            ("/bin/dotnet", "dotnet bin/Debug/App.dll"),
            ("/bin/php", "php artisan serve"),
            ("/bin/ruby", "ruby bin/rails server"),
            ("/work/app/target/debug/api", "/work/app/target/debug/api"),
            ("/tmp/go-build123/b001/exe/api", "/tmp/go-build123/b001/exe/api"),
        ] { assert_eq!(kind(exe, command), ProcessKind::Service, "{command}"); }
    }

    #[test]
    fn missing_metadata_and_legacy_snapshots_stay_unknown() {
        assert_eq!(
            classify(
                None,
                "node server.js",
                Some("/work/app"),
                &[Path::new("/work/app")]
            ),
            ProcessKind::Unknown
        );
        assert_eq!(
            classify(Some("/bin/node"), "node server.js", None, &[]),
            ProcessKind::Unknown
        );
        assert_eq!(
            kind("/bin/node", "node .venv/tool.js"),
            ProcessKind::Unknown
        );
        let bindings = crate::process::parse_ports("p123\ncnode\nf1\ntIPv4\nn127.0.0.1:3000\n");
        let mut json = serde_json::to_value(&bindings[0]).unwrap();
        json.as_object_mut().unwrap().remove("process_kind");
        let old: crate::model::PortBinding = serde_json::from_value(json).unwrap();
        assert_eq!(old.process_kind, ProcessKind::Unknown);
    }
}
