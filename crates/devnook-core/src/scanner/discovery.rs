use crate::model::{Candidate, ComponentCandidate};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) fn group(
    root: &Path,
    raw: Vec<Candidate>,
    boundaries: &BTreeMap<PathBuf, &'static str>,
    imported: &HashSet<String>,
    clients: &HashSet<PathBuf>,
) -> Vec<Candidate> {
    let manifests: HashSet<_> = raw
        .iter()
        .map(|c| PathBuf::from(&c.canonical_path))
        .collect();
    let mut pairs = BTreeMap::<PathBuf, (bool, bool)>::new();
    for candidate in &raw {
        let path = Path::new(&candidate.canonical_path);
        if boundaries.contains_key(path) {
            continue;
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        let pair = pairs.entry(parent.into()).or_default();
        match path.file_name().and_then(|name| name.to_str()) {
            Some("frontend" | "front-end" | "web" | "client") => pair.0 = true,
            Some("backend" | "back-end" | "server" | "api") => pair.1 = true,
            _ => {}
        }
    }
    // Directory names are weaker evidence than a repository/workspace boundary.
    // Do not turn a collection containing unrelated projects into one full-stack project.
    let mut unrelated = HashSet::new();
    for candidate in &raw {
        let path = Path::new(&candidate.canonical_path);
        for parent in path.ancestors().skip(1).take_while(|p| p.starts_with(root)) {
            if !pairs
                .get(parent)
                .is_some_and(|(front, back)| *front && *back)
            {
                continue;
            }
            let first = path
                .strip_prefix(parent)
                .unwrap()
                .iter()
                .next()
                .and_then(|name| name.to_str());
            if !matches!(
                first,
                Some(
                    "frontend"
                        | "front-end"
                        | "web"
                        | "client"
                        | "backend"
                        | "back-end"
                        | "server"
                        | "api"
                        | "packages"
                        | "libs"
                        | "shared"
                        | "common"
                        | "workers"
                        | "services"
                        | "tools"
                )
            ) {
                unrelated.insert(parent.to_path_buf());
            }
        }
    }
    let combined: HashSet<_> = pairs
        .into_iter()
        .filter_map(|(path, (front, back))| {
            (front && back && !unrelated.contains(&path)).then_some(path)
        })
        .collect();
    let owner_of = |path: &Path| {
        let ancestors: Vec<_> = path
            .ancestors()
            .take_while(|p| p.starts_with(root))
            .collect();
        ["GIT", "WORKSPACE", "CLIENT"]
            .iter()
            .find_map(|kind| {
                ancestors
                    .iter()
                    .copied()
                    .find(|p| boundaries.get(*p) == Some(kind))
            })
            .or_else(|| ancestors.iter().copied().find(|p| combined.contains(*p)))
            .or_else(|| {
                ancestors
                    .iter()
                    .rev()
                    .copied()
                    .find(|p| manifests.contains(*p))
            })
            .unwrap_or(path)
            .to_path_buf()
    };
    let client_owners: HashSet<_> = clients.iter().map(|path| owner_of(path)).collect();
    let mut groups = BTreeMap::<PathBuf, Vec<Candidate>>::new();
    for candidate in raw {
        let owner = owner_of(Path::new(&candidate.canonical_path));
        groups.entry(owner).or_default().push(candidate);
    }
    let mut projects = Vec::new();
    for (owner, mut members) in groups {
        members.sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));
        let owner_text = owner.to_string_lossy().into_owned();
        let native_group = client_owners.contains(&owner);
        // Structural roots and shared libraries alone must not resurrect a native app.
        if native_group && !members.iter().any(|c| c.runnable) {
            continue;
        }
        let boundary = boundaries.get(&owner).copied().unwrap_or_else(|| {
            if combined.contains(&owner) {
                "COMBINED"
            } else {
                "MANIFEST"
            }
        });
        let boundary = if boundary == "CLIENT" {
            "WORKSPACE"
        } else {
            boundary
        };
        let mut project = members
            .iter()
            .find(|candidate| candidate.canonical_path == owner_text)
            .cloned()
            .unwrap_or_else(|| Candidate {
                id: Uuid::new_v4().to_string(),
                name: owner
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                root_path: owner_text.clone(),
                canonical_path: owner_text.clone(),
                project_type: "MIXED".into(),
                frameworks: vec![],
                build_tools: vec![],
                evidence: vec![],
                is_git: boundary == "GIT",
                is_monorepo: false,
                parent_path: None,
                runnable: false,
                needs_review: false,
                imported: false,
                components: vec![],
                boundary: String::new(),
            });
        project.components = members
            .iter()
            .filter(|candidate| candidate.canonical_path != owner_text || candidate.runnable)
            .map(|candidate| ComponentCandidate {
                name: if candidate.canonical_path == owner_text {
                    "根目录入口".into()
                } else {
                    Path::new(&candidate.canonical_path)
                        .strip_prefix(&owner)
                        .unwrap_or(Path::new(&candidate.name))
                        .to_string_lossy()
                        .into()
                },
                canonical_path: candidate.canonical_path.clone(),
                project_type: candidate.project_type.clone(),
                frameworks: candidate.frameworks.clone(),
                build_tools: candidate.build_tools.clone(),
                evidence: candidate.evidence.clone(),
                runnable: candidate.runnable,
                needs_review: candidate.needs_review,
                imported: imported.contains(&candidate.canonical_path),
            })
            .collect();
        let mut types: Vec<_> = members
            .iter()
            .filter(|c| !native_group || c.canonical_path != owner_text || c.runnable)
            .map(|c| c.project_type.clone())
            .collect();
        types.sort();
        types.dedup();
        project.project_type = if types.len() == 1 {
            types.remove(0)
        } else {
            "MIXED".into()
        };
        for member in &members {
            project.frameworks.extend(member.frameworks.iter().cloned());
            project
                .build_tools
                .extend(member.build_tools.iter().cloned());
            let relative = Path::new(&member.canonical_path)
                .strip_prefix(&owner)
                .unwrap();
            for evidence in &member.evidence {
                project
                    .evidence
                    .push(relative.join(evidence).to_string_lossy().into());
            }
        }
        if boundary == "GIT" {
            project.evidence.push(".git".into());
        } else if boundary == "WORKSPACE" && super::plain_file(&owner.join("pnpm-workspace.yaml")) {
            project.evidence.push("pnpm-workspace.yaml".into());
        }
        project.frameworks.sort();
        project.frameworks.dedup();
        project.build_tools.sort();
        project.build_tools.dedup();
        project.evidence.sort();
        project.evidence.dedup();
        project.needs_review |= members.iter().any(|c| c.needs_review);
        project.is_monorepo |= members.len() > 1 || boundary == "WORKSPACE";
        project.imported = imported.contains(&owner_text);
        project.boundary = boundary.into();
        projects.push(project);
    }
    let paths: HashSet<_> = projects
        .iter()
        .map(|c| PathBuf::from(&c.canonical_path))
        .collect();
    for project in &mut projects {
        project.parent_path = Path::new(&project.canonical_path)
            .parent()
            .and_then(|parent| {
                parent
                    .ancestors()
                    .find(|ancestor| paths.contains(*ancestor))
            })
            .map(|path| path.to_string_lossy().into());
    }
    projects
}
