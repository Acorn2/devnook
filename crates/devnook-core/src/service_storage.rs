use crate::{model::now, service_model::*, storage};
use rusqlite::{params, Connection, OptionalExtension};

pub fn definitions(db: &Connection) -> Result<Vec<ServiceDefinition>, String> {
    let mut stmt = db
        .prepare("SELECT payload FROM service_definitions WHERE archived=0 ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|row| {
        serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    })
    .collect()
}
pub fn definition(db: &Connection, id: &str) -> Result<ServiceDefinition, String> {
    let value: Option<String> = db
        .query_row(
            "SELECT payload FROM service_definitions WHERE id=?1 AND archived=0",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&value.ok_or("服务配置不存在或已归档")?).map_err(|e| e.to_string())
}
pub fn save_definition(db: &Connection, value: &ServiceDefinition) -> Result<(), String> {
    db.execute("INSERT INTO service_definitions(id,project_id,payload) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,payload=excluded.payload", params![value.id, value.project_id, serde_json::to_string(value).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
    storage::audit(db, "save_service", &value.id, &value.revision)
}
pub fn runs(db: &Connection, service_id: &str, limit: usize) -> Result<Vec<ServiceRun>, String> {
    let mut stmt = db
        .prepare("SELECT payload FROM service_runs WHERE service_id=?1 ORDER BY seq DESC LIMIT ?2")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![service_id, limit.min(100)], |r| {
            r.get::<_, String>(0)
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
pub fn run(db: &Connection, id: &str) -> Result<ServiceRun, String> {
    let value: String = db
        .query_row("SELECT payload FROM service_runs WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .map_err(|_| "运行记录不存在".to_string())?;
    serde_json::from_str(&value).map_err(|e| e.to_string())
}
pub fn active_runs(db: &Connection) -> Result<Vec<ServiceRun>, String> {
    let mut stmt = db.prepare("SELECT payload FROM service_runs WHERE state IN ('STARTING','RUNNING','READY','STOPPING','UNKNOWN')").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
pub fn save_run(db: &Connection, value: &ServiceRun) -> Result<(), String> {
    let state = serde_json::to_value(&value.state).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO service_runs(id,service_id,state,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET state=excluded.state,payload=excluded.payload", params![value.id, value.service_id, state.as_str(), serde_json::to_string(value).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn archive(db: &Connection, id: &str) -> Result<(), String> {
    db.execute(
        "UPDATE service_definitions SET archived=1 WHERE id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    storage::audit(db, "archive_service", id, "history_preserved")
}
pub fn failed_run(
    service: &ServiceDefinition,
    state: ServiceState,
    error: String,
    log_path: String,
) -> ServiceRun {
    ServiceRun {
        id: uuid::Uuid::new_v4().to_string(),
        service_id: service.id.clone(),
        project_id: service.project_id.clone(),
        revision: service.revision.clone(),
        launch_source: "DEVNOOK".into(),
        workspace_run_id: None,
        state,
        pid: None,
        worker_pid: None,
        pgid: None,
        worker_start: None,
        started_at: now(),
        stopped_at: Some(now()),
        ready_at: None,
        exit_code: None,
        error: Some(error),
        log_path,
        stop_requested: false,
        stop_requested_at_ms: None,
        log_truncated: false,
        memory_rss_bytes: None,
        cpu_time_ms: None,
    }
}
