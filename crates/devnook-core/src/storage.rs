use crate::model::{now, Candidate, Project, ProjectEdit, Settings};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub fn open(path: &Path) -> Result<Connection, String> {
    let existed = path.exists();
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, canonical_path TEXT NOT NULL UNIQUE, payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS app_settings (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS process_assignments (fingerprint TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS operation_audits (id TEXT PRIMARY KEY, action TEXT NOT NULL, subject TEXT NOT NULL, result TEXT NOT NULL, created_at INTEGER NOT NULL);
        ").map_err(|e| e.to_string())?;
    let version: u32 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > 3 {
        return Err("数据库版本高于当前 DevNook，请升级应用".into());
    }
    if version < 2 {
        if existed && version == 1 {
            let backup = path.with_extension("phase1-backup.db");
            if !backup.exists() {
                db.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])
                    .map_err(|e| format!("Phase 1 数据备份失败，已停止迁移：{e}"))?;
            }
        }
        db.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS service_definitions (id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id) ON DELETE SET NULL, archived INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS service_runs (seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, service_id TEXT NOT NULL REFERENCES service_definitions(id), state TEXT NOT NULL, payload TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS service_runs_service_seq ON service_runs(service_id, seq DESC);
            PRAGMA user_version=2; COMMIT;").map_err(|e| e.to_string())?;
    }
    if version < 3 {
        if existed && version == 2 {
            let backup = path.with_extension("phase2-backup.db");
            if !backup.exists() {
                db.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])
                    .map_err(|e| format!("Phase 2 数据备份失败，已停止迁移：{e}"))?;
            }
        }
        db.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE workspace_groups (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
            CREATE TABLE workspace_runs (seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL, workspace_id TEXT NOT NULL, state TEXT NOT NULL, payload TEXT NOT NULL);
            CREATE INDEX workspace_runs_group_seq ON workspace_runs(workspace_id,seq DESC);
            CREATE TABLE agent_grants (id TEXT PRIMARY KEY, token_hash TEXT UNIQUE NOT NULL, payload TEXT NOT NULL);
            CREATE TABLE access_audits (seq INTEGER PRIMARY KEY AUTOINCREMENT, origin TEXT NOT NULL, client_id TEXT NOT NULL, action TEXT NOT NULL, subject TEXT NOT NULL, result TEXT NOT NULL, created_at INTEGER NOT NULL);
            PRAGMA user_version=3; COMMIT;").map_err(|e| e.to_string())?;
    }
    Ok(db)
}

pub fn list(db: &Connection) -> Result<Vec<Project>, String> {
    let mut stmt = db
        .prepare("SELECT payload FROM projects ORDER BY canonical_path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        serde_json::from_str(&r.map_err(|e| e.to_string())?)
            .map_err(|e| format!("项目档案损坏：{e}"))
    })
    .collect()
}

pub fn import(
    db: &mut Connection,
    candidates: Vec<Candidate>,
    settings: &Settings,
    complete: bool,
) -> Result<Vec<Project>, String> {
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for mut candidate in candidates {
        let existing: Option<String> = tx
            .query_row(
                "SELECT payload FROM projects WHERE canonical_path=?1",
                [&candidate.canonical_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(payload) = existing {
            let mut project: Project =
                serde_json::from_str(&payload).map_err(|e| format!("项目档案损坏：{e}"))?;
            if complete {
                candidate.id = project.candidate.id.clone();
                candidate.name = project.candidate.name.clone();
                candidate.imported = true;
                project.candidate = candidate;
            } else {
                // A partial scan must not erase components outside its coverage.
                for component in candidate.components {
                    if let Some(previous) = project
                        .candidate
                        .components
                        .iter_mut()
                        .find(|previous| previous.canonical_path == component.canonical_path)
                    {
                        *previous = component;
                    } else {
                        project.candidate.components.push(component);
                    }
                }
                project.candidate.boundary = candidate.boundary;
                project.candidate.is_monorepo |= candidate.is_monorepo;
            }
            project
                .candidate
                .components
                .sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));
            project.updated_at = now();
            tx.execute(
                "UPDATE projects SET payload=?1 WHERE id=?2",
                params![
                    serde_json::to_string(&project).map_err(|e| e.to_string())?,
                    project.candidate.id
                ],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }
        candidate.id = Uuid::new_v4().to_string();
        candidate.imported = true;
        let project = Project {
            default_ide: settings.default_ide.clone(),
            candidate,
            is_favorite: false,
            category: String::new(),
            created_at: now(),
            updated_at: now(),
        };
        tx.execute(
            "INSERT OR IGNORE INTO projects(id,canonical_path,payload) VALUES (?1,?2,?3)",
            params![
                project.candidate.id,
                project.candidate.canonical_path,
                serde_json::to_string(&project).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    list(db)
}

pub fn edit(db: &Connection, edit: ProjectEdit) -> Result<Vec<Project>, String> {
    if edit.name.trim().is_empty() || edit.name.len() > 160 || edit.category.len() > 80 {
        return Err("项目名称或分类长度不合法".into());
    }
    validate_ide(&edit.default_ide)?;
    let mut project = list(db)?
        .into_iter()
        .find(|p| p.candidate.id == edit.id)
        .ok_or("项目记录不存在")?;
    project.candidate.name = edit.name.trim().into();
    project.default_ide = edit.default_ide;
    project.category = edit.category.trim().into();
    project.is_favorite = edit.is_favorite;
    project.updated_at = now();
    db.execute(
        "UPDATE projects SET payload=?1 WHERE id=?2",
        params![
            serde_json::to_string(&project).map_err(|e| e.to_string())?,
            edit.id
        ],
    )
    .map_err(|e| e.to_string())?;
    list(db)
}

pub fn settings(db: &Connection) -> Result<Settings, String> {
    let mut stmt = db
        .prepare("SELECT payload FROM app_settings WHERE id=1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => serde_json::from_str(&row.get::<_, String>(0).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string()),
        None => Ok(Settings::default()),
    }
}

pub fn validate_ide(ide: &str) -> Result<(), String> {
    crate::ide::validate_target(ide)
}

const REMEMBERED_APPS: &str = "remembered_development_apps";
fn settings_payload(db: &Connection) -> Result<serde_json::Value, String> {
    let payload: Option<String> = db
        .query_row("SELECT payload FROM app_settings WHERE id=1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let value = match payload {
        Some(payload) => serde_json::from_str(&payload).map_err(|_| "设置数据损坏")?,
        None => serde_json::to_value(Settings::default()).map_err(|e| e.to_string())?,
    };
    if !value.is_object() {
        return Err("设置数据格式无效".into());
    }
    Ok(value)
}
fn write_settings_payload(db: &Connection, value: &serde_json::Value) -> Result<(), String> {
    db.execute("INSERT INTO app_settings(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
        [serde_json::to_string(value).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn remembered_apps(db: &Connection) -> Result<Vec<crate::ide::RememberedApp>, String> {
    let payload = settings_payload(db)?;
    payload
        .get(REMEMBERED_APPS)
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|_| "已记住的应用记录损坏".into())
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}
pub fn remember_app(db: &Connection, app: &crate::ide::DevelopmentApp) -> Result<(), String> {
    let mut payload = settings_payload(db)?;
    let mut apps: Vec<crate::ide::RememberedApp> = payload
        .get(REMEMBERED_APPS)
        .map(|value| serde_json::from_value(value.clone()).map_err(|_| "已记住的应用记录损坏"))
        .transpose()?
        .unwrap_or_default();
    let saved = crate::ide::RememberedApp {
        path: app.path.clone(),
        bundle_id: app.bundle_id.clone(),
    };
    if let Some(existing) = apps.iter_mut().find(|saved| saved.path == app.path) {
        *existing = saved;
    } else {
        if apps.len() >= 256 {
            return Err("已记住的应用达到 256 个上限".into());
        }
        apps.push(saved);
    }
    payload[REMEMBERED_APPS] = serde_json::to_value(apps).map_err(|e| e.to_string())?;
    write_settings_payload(db, &payload)
}

pub fn save_settings(db: &Connection, value: &Settings) -> Result<(), String> {
    validate_ide(&value.default_ide)?;
    crate::ide::validate_terminal(&value.default_terminal)?;
    if !(1..=12).contains(&value.scan_depth) || !(2..=60).contains(&value.refresh_seconds) {
        return Err("扫描深度应为 1–12，刷新间隔应为 2–60 秒".into());
    }
    for app in [&value.idea_path, &value.vscode_path, &value.cursor_path] {
        if !app.is_empty()
            && (!Path::new(app).is_absolute() || !app.ends_with(".app") || !Path::new(app).is_dir())
        {
            return Err("自定义应用必须是已存在的绝对 .app 路径".into());
        }
    }
    // The local app registry is independent of editable settings. A stale form or
    // older settings payload must not erase an application selected meanwhile.
    let mut payload = settings_payload(db)?;
    for (key, value) in serde_json::to_value(value)
        .map_err(|e| e.to_string())?
        .as_object()
        .ok_or("设置格式无效")?
    {
        payload[key] = value.clone();
    }
    write_settings_payload(db, &payload)
}

pub fn audit(db: &Connection, action: &str, subject: &str, result: &str) -> Result<(), String> {
    db.execute(
        "INSERT INTO operation_audits VALUES(?1,?2,?3,?4,?5)",
        params![Uuid::new_v4().to_string(), action, subject, result, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
