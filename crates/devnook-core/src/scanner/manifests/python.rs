use super::{lex, Token};
use crate::scanner::{git_boundary, plain_file, read_manifest};
use std::{fs, net::IpAddr, path::Path};
use Token::{Indent, Line, String as Quoted, Symbol, Word};

fn body<'a, 'body>(
    statements: &'body [(usize, &'a [Token])],
) -> impl Iterator<Item = &'a [Token]> + 'body
where
    'a: 'body,
{
    let indent = statements.first().map(|(indent, _)| *indent).unwrap_or(0);
    statements
        .iter()
        .take_while(move |(level, _)| *level > 0 && *level >= indent)
        .filter_map(move |(level, statement)| (*level == indent).then_some(*statement))
}

fn call<'a>(statement: &'a [Token], name: &str) -> Option<Vec<&'a [Token]>> {
    let [Word(target), Symbol('('), inner @ .., Symbol(')')] = statement else {
        return None;
    };
    if target != name {
        return None;
    }
    let mut depth = 0usize;
    let mut start = 0;
    let mut args = Vec::new();
    for (index, token) in inner.iter().enumerate() {
        match token {
            Symbol('(' | '[' | '{') => depth += 1,
            Symbol(')' | ']' | '}') => depth = depth.checked_sub(1)?,
            Symbol(',') if depth == 0 => {
                args.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    if start < inner.len() {
        args.push(&inner[start..]);
    }
    if args.iter().any(|arg| {
        arg.is_empty() || matches!(arg.first(), Some(Word(word)) if word.starts_with('*'))
    }) {
        return None;
    }
    Some(args)
}

fn keyword<'a>(args: &[&'a [Token]], name: &str) -> Option<&'a [Token]> {
    args.iter().find_map(|argument| match argument {
        [Word(key), Symbol('='), value @ ..] if key == name => Some(value),
        _ => None,
    })
}

fn argument_default<'a>(statements: &[&'a [Token]], name: &str) -> Option<&'a [Token]> {
    let mut arguments = statements.iter();
    let parser = arguments.position(|statement| {
        matches!(statement, [Word(name), Symbol('='), value @ ..]
            if name == "parser" && call(value, "argparse.ArgumentParser").is_some())
    })?;
    let parse = arguments.position(|statement| {
        matches!(statement, [Word(name), Symbol('='), Word(method), Symbol('('), Symbol(')')]
            if name == "args" && method == "parser.parse_args")
    })?;
    let mut defaults = statements[parser + 1..parser + 1 + parse]
        .iter()
        .filter_map(|statement| call(statement, "parser.add_argument"))
        .filter(|args| matches!(args.first(), Some([Quoted(flag)]) if flag == name));
    let args = defaults.next()?;
    if defaults.next().is_some() || keyword(&args, "dest").is_some() {
        return None;
    }
    keyword(&args, "default")
}

fn launch_port(text: &str) -> Option<Option<u16>> {
    let tokens = lex(text, true);
    let statements: Vec<_> = tokens
        .split(|token| *token == Line)
        .filter_map(|line| match line {
            [Indent(indent), statement @ ..] if !statement.is_empty() => Some((*indent, statement)),
            _ => None,
        })
        .collect();
    let top_level = statements.iter().filter(|(indent, _)| *indent == 0);
    if !top_level.clone().any(|(_, statement)| {
        matches!(statement, [Word(import), Word(module)] if import == "import" && module == "uvicorn")
    }) {
        return None;
    }
    let guard = statements.iter().position(|(indent, statement)| {
        *indent == 0
            && matches!(statement,
            [Word(condition), Word(name), Symbol('='), Symbol('='), Quoted(value), Symbol(':')]
            if condition == "if" && name == "__name__" && value == "__main__")
    })?;
    let guarded: Vec<_> = body(&statements[guard + 1..]).collect();
    let direct_main = guarded.iter().any(
        |statement| matches!(statement, [Word(name), Symbol('('), Symbol(')')] if name == "main"),
    );
    let launch_body: Vec<_> = if direct_main {
        let main = statements[..guard].iter().rposition(|(indent, statement)| {
            *indent == 0 && matches!(statement,
                [Word(define), Word(name), Symbol('('), Symbol(')'), tail @ ..]
                if define == "def" && name == "main" && match tail {
                    [Symbol(':')] => true,
                    [Word(arrow), Word(return_type), Symbol(':')] => arrow == "->" && return_type == "None",
                    _ => false,
                })
        })?;
        body(&statements[main + 1..guard]).collect()
    } else {
        guarded
    };
    let mut launches = launch_body
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| call(statement, "uvicorn.run").map(|args| (index, args)));
    let (index, args) = launches.next()?;
    if launches.next().is_some() || args.is_empty() {
        return None;
    }
    let before_launch = &launch_body[..index];
    if before_launch
        .iter()
        .filter_map(|statement| call(statement, "parser.add_argument"))
        .any(|args| {
            !matches!(args.first(), Some([Quoted(flag)]) if flag.starts_with('-'))
                || keyword(&args, "required")
                    .is_some_and(|value| !matches!(value, [Word(flag)] if flag == "False"))
        })
    {
        return None;
    }
    if let Some(host) = keyword(&args, "host") {
        let host = match host {
            [Word(value)] if value == "args.host" => argument_default(before_launch, "--host")?,
            literal => literal,
        };
        if !matches!(host, [Quoted(value)] if value == "localhost" || value.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
        {
            return None;
        }
    }
    let port = keyword(&args, "port").and_then(|value| {
        let value = match value {
            [Word(value)] if value == "args.port" => argument_default(before_launch, "--port")?,
            literal => literal,
        };
        match value {
            [Word(value)] => value.parse::<u16>().ok().filter(|port| *port != 0),
            _ => None,
        }
    });
    Some(port)
}

pub(crate) fn uvicorn_entrypoint(path: &Path) -> Option<(Vec<String>, String, Option<u16>)> {
    let mut entries = ["server", "api", "backend", "app"]
        .into_iter()
        .filter_map(|module| {
            let dir = path.join(module);
            if !fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir())
                || git_boundary(&dir)
                || !plain_file(&dir.join("__init__.py"))
            {
                return None;
            }
            Some((
                vec!["-m".into(), module.into()],
                format!("{module}/__main__.py"),
            ))
        })
        .chain([(vec!["main.py".into()], "main.py".into())])
        .filter_map(|(args, source)| {
            let text = read_manifest(&path.join(&source))?;
            Some((args, source, launch_port(&text)?))
        });
    let entry = entries.next()?;
    entries.next().is_none().then_some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECT: &str = "import uvicorn\nif __name__ == '__main__':\n    uvicorn.run('api:app', port=8001, host='::1')\n";

    #[test]
    fn literal_port_is_optional_and_nonlocal_host_is_not_suggested() {
        assert_eq!(launch_port(DIRECT), Some(Some(8001)));
        assert_eq!(
            launch_port(&DIRECT.replace("8001", "int(config.port)")),
            Some(None)
        );
        assert_eq!(launch_port(&DIRECT.replace("8001", "65536")), Some(None));
        assert_eq!(launch_port(&DIRECT.replace("'::1'", "'0.0.0.0'")), None);
        assert_eq!(launch_port(&DIRECT.replace("'::1'", "config.host")), None);
    }

    #[test]
    fn ignores_nonexecuting_and_ambiguous_evidence() {
        assert_eq!(launch_port(&format!("\"\"\"\n{DIRECT}\"\"\"\n")), None);
        let comments: Vec<_> = DIRECT.lines().map(|line| format!("# {line}")).collect();
        assert_eq!(launch_port(&comments.join("\n")), None);
        assert_eq!(
            launch_port("import uvicorn\ndef main():\n    uvicorn.run('api:app', port=8001)\n"),
            None
        );
        assert_eq!(
            launch_port(&format!(
                "{DIRECT}    uvicorn.run('other:app', port=8002)\n"
            )),
            None
        );
        let required = DIRECT.replace(
            "    uvicorn.run",
            "    parser.add_argument('--root', required=True)\n    uvicorn.run",
        );
        assert_eq!(launch_port(&required), None);
    }

    #[test]
    fn uses_a_unique_local_entrypoint_and_skips_linked_or_nested_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let sibling = temp.path().join("external");
        fs::create_dir(&project).unwrap();
        fs::create_dir(&sibling).unwrap();
        fs::write(sibling.join("__init__.py"), "").unwrap();
        fs::write(sibling.join("__main__.py"), DIRECT).unwrap();
        std::os::unix::fs::symlink(&sibling, project.join("server")).unwrap();
        assert!(uvicorn_entrypoint(&project).is_none());
        std::os::unix::fs::symlink(sibling.join("__main__.py"), project.join("main.py")).unwrap();
        assert!(uvicorn_entrypoint(&project).is_none());
        fs::rename(project.join("main.py"), project.join("symlink.py")).unwrap();
        fs::write(project.join("main.py"), DIRECT).unwrap();
        assert_eq!(
            uvicorn_entrypoint(&project),
            Some((vec!["main.py".into()], "main.py".into(), Some(8001)))
        );
        fs::create_dir(project.join("api")).unwrap();
        fs::write(project.join("api/__init__.py"), "").unwrap();
        fs::write(project.join("api/__main__.py"), DIRECT).unwrap();
        assert!(uvicorn_entrypoint(&project).is_none());
        fs::create_dir(project.join("api/.git")).unwrap();
        assert!(uvicorn_entrypoint(&project).is_some());
    }
}
