//! Static discovery only: none of these project files is evaluated or executed.
use super::{plain_file, read_manifest};
use quick_xml::{events::Event, Reader};
use std::path::Path;

#[derive(Default)]
pub(super) struct Hints {
    pub purpose: super::purpose::Purpose,
    pub types: Vec<&'static str>,
    pub tools: Vec<String>,
    pub frameworks: Vec<String>,
    pub evidence: Vec<String>,
    pub runnable: bool,
    pub review: bool,
    pub workspace: bool,
    pub modules: Vec<String>,
    pub excludes: Vec<String>,
    pub warnings: Vec<String>,
}
impl Hints {
    fn project(&mut self, kind: &'static str, tool: &str, manifest: &str) {
        self.types.push(kind);
        self.tools.push(tool.into());
        self.evidence.push(manifest.into());
    }
    fn problem(&mut self, file: &str, reason: &str) {
        self.review = true;
        if self.warnings.len() < 8 {
            self.warnings
                .push(format!("{file}：{reason}，可选择具体目录或使用深度补扫"));
        }
    }
    fn patterns(&mut self, values: impl IntoIterator<Item = String>) {
        for value in values {
            if let Some(excluded) = value.strip_prefix('!') {
                self.excludes.push(excluded.into());
            } else {
                self.modules.push(value);
            }
        }
    }
}

fn strings(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn has_suffix(text: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|suffix| text.ends_with(suffix))
}

// A deliberately small literal lexer. It ignores comments and never evaluates expressions.
#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    String(String),
    Symbol(char),
    Line,
}
fn tokens(text: &str) -> Vec<Token> {
    let mut chars = text.chars().peekable();
    let mut result = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            '\n' => result.push(Token::Line),
            c if c.is_whitespace() => {}
            '#' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                result.push(Token::Line);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                result.push(Token::Line);
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
            }
            '\'' | '"' | '`' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == c {
                        closed = true;
                        break;
                    }
                    if next == '\\' && chars.peek().is_some_and(|v| *v == c || *v == '\\') {
                        value.push(chars.next().unwrap());
                    } else {
                        value.push(next);
                    }
                }
                if closed {
                    result.push(Token::String(value));
                } else {
                    result.push(Token::Symbol('?'));
                }
            }
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | '=' | ';' => result.push(Token::Symbol(c)),
            _ => {
                let mut word = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || "()[]{},=;\"'`#".contains(next) {
                        break;
                    }
                    word.push(chars.next().unwrap());
                }
                result.push(Token::Word(word));
            }
        }
    }
    result
}
pub(super) fn static_tokens(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter_map(|t| match t {
            Token::Word(s) | Token::String(s) => Some(s),
            _ => None,
        })
        .collect()
}
pub(super) fn literal_calls(text: &str, name: &str) -> Vec<Vec<String>> {
    calls(text, name, false).0
}
fn literal(value: &Token) -> Option<String> {
    match value {
        Token::String(s) | Token::Word(s) if !s.contains(['$', '{', '}', '\n']) => Some(s.clone()),
        _ => None,
    }
}
fn yaml_literal(value: &Token) -> Option<String> {
    if matches!(value, Token::Word(s) if s.starts_with(['*', '&', '!'])) {
        None
    } else {
        literal(value)
    }
}
fn calls(text: &str, name: &str, bare: bool) -> (Vec<Vec<String>>, bool) {
    let ts = tokens(text);
    let mut found = Vec::new();
    let mut dynamic = false;
    for (i, token) in ts.iter().enumerate() {
        if token != &Token::Word(name.into()) {
            continue;
        }
        let parenthesized = ts.get(i + 1) == Some(&Token::Symbol('('));
        if !parenthesized && !bare {
            dynamic = true;
            continue;
        }
        let mut values = Vec::new();
        let mut valid = true;
        for t in ts.iter().skip(i + 1 + usize::from(parenthesized)) {
            match t {
                Token::Symbol(')') if parenthesized => break,
                Token::Line | Token::Symbol(';') if !parenthesized => break,
                Token::Line | Token::Symbol(',') => {}
                _ => {
                    if (!bare || matches!(t, Token::String(_))) && literal(t).is_some() {
                        values.push(literal(t).unwrap());
                    } else {
                        valid = false;
                        break;
                    }
                }
            }
        }
        if valid && !values.is_empty() {
            found.push(values);
        } else {
            dynamic = true;
        }
    }
    (found, dynamic)
}

fn yaml_list(text: &str, key: &str) -> Result<Vec<String>, ()> {
    let mut active = false;
    let mut values = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            if active {
                break;
            }
            let Some(value) = trimmed.strip_prefix(&format!("{key}:")) else {
                continue;
            };
            active = true;
            if !value.trim().is_empty() && !value.trim().starts_with('#') {
                let ts: Vec<_> = tokens(value)
                    .into_iter()
                    .filter(|t| *t != Token::Line)
                    .collect();
                if ts.first() != Some(&Token::Symbol('[')) || ts.last() != Some(&Token::Symbol(']'))
                {
                    return Err(());
                }
                for token in &ts[1..ts.len() - 1] {
                    if token == &Token::Symbol(',') {
                        continue;
                    }
                    values.push(yaml_literal(token).ok_or(())?);
                }
                break;
            }
        } else if active {
            let item = trimmed.strip_prefix("- ").ok_or(())?;
            let ts = tokens(item);
            let ts: Vec<_> = ts.iter().filter(|v| **v != Token::Line).collect();
            if ts.len() != 1 {
                return Err(());
            }
            values.push(yaml_literal(ts[0]).ok_or(())?);
        }
    }
    if values
        .iter()
        .any(|s| s.starts_with('&') || s.contains(['{', '}']))
    {
        return Err(());
    }
    Ok(values)
}

fn xml_modules(text: &str) -> Result<Vec<String>, ()> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::new();
    let mut result = Vec::new();
    let mut module_text: Option<String> = None;
    loop {
        let event = reader.read_event().map_err(|_| ())?;
        let is_start = matches!(&event, Event::Start(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.local_name().as_ref().to_owned();
                if is_start && name == "module" && stack.last().is_some_and(|s| s == "modules") {
                    module_text = Some(String::new());
                }
                if matches!(name.as_str(), "ProjectReference" | "Project") {
                    for attr in e.attributes() {
                        let attr = attr.map_err(|_| ())?;
                        if ["Include", "Path"].contains(&attr.key.as_ref()) {
                            let value = attr
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|_| ())?
                                .into_owned();
                            if has_suffix(&value, &[".csproj", ".fsproj", ".vbproj"]) {
                                result.push(value.replace('\\', "/"));
                            }
                        }
                    }
                }
                if is_start {
                    stack.push(name);
                }
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_owned();
                if name == "module" {
                    if let Some(text) = module_text.take() {
                        result.push(
                            quick_xml::escape::unescape(&text)
                                .map_err(|_| ())?
                                .trim()
                                .to_owned(),
                        );
                    }
                }
                while let Some(top) = stack.pop() {
                    if top == name {
                        break;
                    }
                }
            }
            Event::Text(e) if module_text.is_some() => module_text
                .as_mut()
                .unwrap()
                .push_str(&e.xml_content(quick_xml::XmlVersion::Implicit1_0)),
            Event::GeneralRef(e) if module_text.is_some() => {
                let text = module_text.as_mut().unwrap();
                text.push('&');
                text.push_str(e.as_ref());
                text.push(';');
            }
            Event::CData(e) if module_text.is_some() => module_text
                .as_mut()
                .unwrap()
                .push_str(&quick_xml::escape::escape(e.as_ref())),
            Event::DocType(_) => return Err(()),
            Event::Eof => {
                if !stack.is_empty() {
                    return Err(());
                }
                break;
            }
            _ => {}
        }
    }
    Ok(result)
}

pub(super) fn analyze(path: &Path, names: &[String]) -> Hints {
    let mut h = Hints::default();
    let pnpm = read_manifest(&path.join("pnpm-workspace.yaml"));
    if let Some(text) = &pnpm {
        h.workspace = true;
        match yaml_list(text, "packages") {
            Ok(v) => h.patterns(v),
            Err(_) => h.problem("pnpm-workspace.yaml", "模块列表不是支持的静态字符串格式"),
        }
    } else if let Some(text) = read_manifest(&path.join("package.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(workspace) = value.get("workspaces") {
                h.workspace = true;
                let entries = workspace
                    .as_array()
                    .or_else(|| workspace.get("packages").and_then(|v| v.as_array()));
                if let Some(entries) = entries {
                    h.patterns(entries.iter().filter_map(|v| v.as_str()).map(str::to_owned));
                } else {
                    h.problem("package.json", "workspaces 格式无法静态识别");
                }
            }
        }
    }
    for file in ["Cargo.toml", "pyproject.toml"] {
        let Some(text) = read_manifest(&path.join(file)) else {
            continue;
        };
        match toml::from_str::<toml::Value>(&text) {
            Ok(value) => {
                let workspace = if file == "Cargo.toml" {
                    value.get("workspace")
                } else {
                    value
                        .get("tool")
                        .and_then(|v| v.get("uv"))
                        .and_then(|v| v.get("workspace"))
                };
                if let Some(workspace) = workspace {
                    h.workspace = true;
                    h.modules.extend(strings(workspace.get("members")));
                    h.excludes.extend(strings(workspace.get("exclude")));
                }
                if file == "Cargo.toml" {
                    for table in std::iter::once(&value).chain(value.get("workspace")) {
                        for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
                            if let Some(deps) = table.get(key).and_then(toml::Value::as_table) {
                                h.modules.extend(
                                    deps.values()
                                        .filter_map(|v| v.get("path"))
                                        .filter_map(toml::Value::as_str)
                                        .map(str::to_owned),
                                );
                            }
                        }
                    }
                }
            }
            Err(_) => h.problem(file, "清单解析失败"),
        }
    }
    if let Some(text) = read_manifest(&path.join("pom.xml")) {
        match xml_modules(&text) {
            Ok(v) => {
                h.workspace |= !v.is_empty();
                h.modules.extend(v);
            }
            Err(_) => h.problem("pom.xml", "XML 模块声明无法安全解析"),
        }
    }
    for file in ["settings.gradle", "settings.gradle.kts"] {
        let Some(text) = read_manifest(&path.join(file)) else {
            continue;
        };
        h.project("JAVA", "Gradle", file);
        h.workspace = true;
        let mut mappings = std::collections::BTreeMap::new();
        let ts: Vec<_> = tokens(&text)
            .into_iter()
            .filter(|t| *t != Token::Line)
            .collect();
        for w in ts.windows(10) {
            if w[0] == Token::Word("project".into())
                && w[1] == Token::Symbol('(')
                && w[3] == Token::Symbol(')')
                && w[4] == Token::Word(".projectDir".into())
                && w[5] == Token::Symbol('=')
                && w[6] == Token::Word("file".into())
                && w[7] == Token::Symbol('(')
                && w[9] == Token::Symbol(')')
            {
                if let (Some(logical), Some(actual)) = (literal(&w[2]), literal(&w[8])) {
                    mappings.insert(logical.trim_start_matches(':').replace(':', "/"), actual);
                }
            }
        }
        for name in ["include", "includeBuild"] {
            let (values, dynamic) = calls(&text, name, true);
            for value in values.into_iter().flatten() {
                let logical = value.trim_start_matches(':').replace(':', "/");
                h.modules
                    .push(mappings.get(&logical).cloned().unwrap_or(logical));
            }
            if dynamic {
                h.problem(file, "存在无法静态识别的模块表达式");
            }
        }
        if text.contains("projectDir") && mappings.is_empty() {
            h.problem(file, "projectDir 不是支持的 file(字面量) 映射");
        }
    }
    if let Some(text) = read_manifest(&path.join("go.work")) {
        h.project("GO", "Go", "go.work");
        h.workspace = true;
        let ts = tokens(&text);
        for (i, token) in ts.iter().enumerate() {
            if token != &Token::Word("use".into()) {
                continue;
            }
            if ts.get(i + 1) == Some(&Token::Symbol('(')) {
                for t in ts
                    .iter()
                    .skip(i + 2)
                    .take_while(|t| **t != Token::Symbol(')'))
                {
                    if let Some(value) = literal(t) {
                        h.modules.push(value);
                    }
                }
            } else if let Some(value) = ts.get(i + 1).and_then(literal) {
                h.modules.push(value);
            }
        }
    }
    for file in ["setup.cfg", "Pipfile", "manage.py"] {
        if plain_file(&path.join(file)) {
            h.project(
                "PYTHON",
                if file == "Pipfile" {
                    "Pipenv"
                } else {
                    "Python"
                },
                file,
            );
        }
    }
    if plain_file(&path.join("manage.py")) {
        h.runnable = true;
        h.frameworks.push("Django".into());
    }
    if let Some(text) = read_manifest(&path.join("composer.json")) {
        h.project("PHP", "Composer", "composer.json");
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            for (key, label) in [
                ("laravel/framework", "Laravel"),
                ("symfony/framework-bundle", "Symfony"),
            ] {
                if v.get("require").and_then(|v| v.get(key)).is_some() {
                    h.frameworks.push(label.into());
                }
            }
            h.runnable |= v.get("scripts").and_then(|v| v.get("start")).is_some()
                || plain_file(&path.join("artisan"))
                || plain_file(&path.join("bin/console"));
        } else {
            h.problem("composer.json", "JSON 解析失败");
        }
    }
    if plain_file(&path.join("Gemfile")) {
        h.project("RUBY", "Bundler", "Gemfile");
    }
    if plain_file(&path.join("config/application.rb")) && plain_file(&path.join("Gemfile")) {
        h.frameworks.push("Rails".into());
        h.runnable = true;
    }
    if let Some(text) = read_manifest(&path.join("pubspec.yaml")) {
        h.project("DART", "Dart", "pubspec.yaml");
        if text.lines().any(|l| l.trim() == "sdk: flutter") {
            h.frameworks.push("Flutter".into());
        }
        h.runnable |=
            plain_file(&path.join("lib/main.dart")) || plain_file(&path.join("bin/main.dart"));
        match yaml_list(&text, "workspace") {
            Ok(v) => {
                h.workspace |= !v.is_empty();
                h.patterns(v);
            }
            Err(_) => h.problem("pubspec.yaml", "workspace 不是支持的静态列表"),
        }
    }
    if let Some(text) = read_manifest(&path.join("Package.swift")) {
        h.project("SWIFT", "SwiftPM", "Package.swift");
        h.runnable |= tokens(&text).windows(2).any(|w| matches!(&w[0], Token::Word(s) if [".executableTarget", ".executable"].contains(&s.as_str())) && w[1] == Token::Symbol('('));
    }
    for (file, tool, call) in [
        ("CMakeLists.txt", "CMake", "add_subdirectory"),
        ("meson.build", "Meson", "subdir"),
    ] {
        let Some(text) = read_manifest(&path.join(file)) else {
            continue;
        };
        h.project("CPP", tool, file);
        h.runnable |= !calls(&text, "add_executable", false).0.is_empty()
            || !calls(&text, "executable", false).0.is_empty();
        let (values, dynamic) = calls(&text, call, false);
        h.modules
            .extend(values.into_iter().filter_map(|v| v.into_iter().next()));
        if dynamic {
            h.problem(file, "存在动态子模块表达式");
        }
    }
    for name in names {
        if has_suffix(name, &[".xcodeproj", ".xcworkspace"])
            && std::fs::symlink_metadata(path.join(name))
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        {
            h.project("SWIFT", "Xcode", name);
        }
        if name.ends_with(".gemspec") && plain_file(&path.join(name)) {
            h.project("RUBY", "RubyGems", name);
        }
        if has_suffix(name, &[".csproj", ".fsproj", ".vbproj", ".sln", ".slnx"]) {
            let Some(text) = read_manifest(&path.join(name)) else {
                continue;
            };
            h.project("DOTNET", ".NET", name);
            h.workspace |= has_suffix(name, &[".sln", ".slnx"]);
            h.runnable |= text.contains("Microsoft.NET.Sdk.Web")
                || text.contains("<OutputType>Exe</OutputType>")
                || text.contains("<OutputType>WinExe</OutputType>");
            if text.contains("Microsoft.NET.Sdk.Web") {
                h.frameworks.push("ASP.NET Core".into());
            }
            let paths = if name.ends_with(".sln") {
                Ok(text
                    .lines()
                    .filter(|l| l.trim_start().starts_with("Project("))
                    .filter_map(|l| l.split('"').nth(5))
                    .map(str::to_owned)
                    .collect::<Vec<_>>())
            } else {
                xml_modules(&text)
            };
            match paths {
                Ok(values) => h.modules.extend(values.into_iter().filter_map(|v| {
                    Path::new(&v.replace('\\', "/"))
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                })),
                Err(_) => h.problem(name, "工程引用 XML 无法解析"),
            }
        }
    }
    if let Some(text) = read_manifest(&path.join(".gitmodules")) {
        for line in text.lines() {
            if let Some(value) = line
                .trim()
                .strip_prefix("path")
                .and_then(|v| v.trim_start().strip_prefix('='))
            {
                h.modules.push(value.trim().trim_matches('"').into());
            }
        }
    }
    if h.modules.len() > 4096 {
        h.modules.truncate(4096);
        h.problem("模块清单", "声明超过 4096 条安全上限");
    }
    if h.excludes.len() > 4096 {
        h.excludes.truncate(4096);
        h.problem("模块清单", "排除声明超过 4096 条安全上限");
    }
    h.purpose = super::purpose::analyze(path, names);
    h
}
