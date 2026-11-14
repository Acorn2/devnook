use crate::{model::now, workspace::ordered, Core};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub origin: String,
    #[serde(default)]
    pub credential: String,
    #[serde(default)]
    pub read_only: bool,
}
impl Default for RequestContext {
    fn default() -> Self {
        Self {
            origin: "GUI".into(),
            credential: String::new(),
            read_only: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrantInput {
    pub name: String,
    pub service_ids: Vec<String>,
    pub workspace_ids: Vec<String>,
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub scan_roots: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGrant {
    pub id: String,
    pub name: String,
    pub services: BTreeMap<String, String>,
    pub workspaces: BTreeMap<String, String>,
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub scan_roots: Vec<String>,
    pub created_at: u64,
    pub revoked: bool,
}
#[derive(Serialize, Deserialize)]
pub struct GrantCreated {
    pub grant: AgentGrant,
    pub token: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessAudit {
    pub origin: String,
    pub client_id: String,
    pub action: String,
    pub subject: String,
    pub result: String,
    pub created_at: u64,
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
impl Core {
    pub fn agent_grants(&self) -> Result<Vec<AgentGrant>, String> {
        let db = self.db.lock().map_err(|_| "数据库锁异常")?;
        let mut statement = db
            .prepare("SELECT payload FROM agent_grants ORDER BY rowid DESC")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn create_agent_grant(&self, input: GrantInput) -> Result<GrantCreated, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "操作锁异常")?;
        if input.name.trim().is_empty()
            || input.name.len() > 80
            || input.service_ids.len() + input.workspace_ids.len() + input.project_ids.len() > 128
        {
            return Err("授权名称必填，范围最多 128 个条目".into());
        }
        let definitions: Vec<_> = self.services()?.into_iter().map(|s| s.definition).collect();
        let mut ids = input.service_ids;
        let mut workspaces = BTreeMap::new();
        for id in input.workspace_ids {
            let group = self
                .workspaces()?
                .into_iter()
                .find(|w| w.id == id)
                .ok_or("授权的工作区不存在")?;
            ids.extend(group.service_ids);
            workspaces.insert(id, group.revision);
        }
        let mut services = BTreeMap::new();
        for id in ordered(&ids, &definitions)? {
            let definition = definitions
                .iter()
                .find(|s| s.id == id)
                .ok_or("服务不存在")?;
            services.insert(id, definition.revision.clone());
        }
        let projects = self.projects()?;
        if input
            .project_ids
            .iter()
            .any(|id| !projects.iter().any(|p| &p.candidate.id == id))
        {
            return Err("授权的项目不存在".into());
        }
        if input.scan_roots.len() > 16 {
            return Err("扫描授权最多 16 个根目录".into());
        }
        let mut scan_roots = Vec::new();
        for root in input.scan_roots {
            let path = std::path::Path::new(&root)
                .canonicalize()
                .map_err(|_| "扫描授权目录不可访问")?;
            if !path.is_dir() {
                return Err("扫描授权必须是目录".into());
            }
            scan_roots.push(path.to_str().ok_or("目录不是 UTF-8")?.to_string());
        }
        if services.is_empty()
            && workspaces.is_empty()
            && input.project_ids.is_empty()
            && scan_roots.is_empty()
        {
            return Err("请选择至少一个授权对象".into());
        }
        let grant = AgentGrant {
            id: Uuid::new_v4().to_string(),
            name: input.name.trim().into(),
            services,
            workspaces,
            project_ids: input.project_ids,
            scan_roots,
            created_at: now(),
            revoked: false,
        };
        let token = format!("dn_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        self.db
            .lock()
            .map_err(|_| "数据库锁异常")?
            .execute(
                "INSERT INTO agent_grants(id,token_hash,payload) VALUES(?1,?2,?3)",
                params![
                    grant.id,
                    digest(&token),
                    serde_json::to_string(&grant).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(GrantCreated { grant, token })
    }
    pub fn revoke_agent_grant(&self, id: &str) -> Result<(), String> {
        // Serialize with writes, including confirmations, so revocation has a precise boundary.
        let mut grant = self
            .agent_grants()?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or("授权不存在")?;
        grant.revoked = true;
        self.db
            .lock()
            .map_err(|_| "数据库锁异常")?
            .execute(
                "UPDATE agent_grants SET payload=?1 WHERE id=?2",
                params![
                    serde_json::to_string(&grant).map_err(|e| e.to_string())?,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn grant(&self, context: &RequestContext) -> Result<AgentGrant, String> {
        if context.read_only || context.credential.is_empty() {
            return Err("MCP 默认为只读；控制操作需要明确授权".into());
        }
        let encoded: String = self
            .db
            .lock()
            .map_err(|_| "数据库锁异常")?
            .query_row(
                "SELECT payload FROM agent_grants WHERE token_hash=?1",
                [digest(&context.credential)],
                |r| r.get(0),
            )
            .map_err(|_| "MCP 授权无效")?;
        let grant: AgentGrant = serde_json::from_str(&encoded).map_err(|_| "授权记录无效")?;
        if grant.revoked {
            return Err("MCP 授权已撤销".into());
        }
        Ok(grant)
    }
    pub(crate) fn authorize(
        &self,
        context: &RequestContext,
        method: &str,
        args: &Value,
    ) -> Result<String, String> {
        if context.origin != "MCP" {
            return if ["GUI", "CLI"].contains(&context.origin.as_str()) {
                Ok(context.origin.clone())
            } else {
                Err("请求来源无效".into())
            };
        }
        if [
            "ping",
            "list_projects",
            "list_services",
            "list_ports",
            "inspect_process",
            "service_history",
            "service_logs",
            "list_workspaces",
            "workspace_history",
        ]
        .contains(&method)
        {
            return Ok("read-only".into());
        }
        let grant = self.grant(context)?;
        let id = args["id"].as_str().unwrap_or_default();
        let check_service = |id: &str| -> Result<(), String> {
            let definitions: Vec<_> = self.services()?.into_iter().map(|s| s.definition).collect();
            for id in ordered(&[id.into()], &definitions)? {
                let definition = definitions
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or("服务不存在")?;
                if grant.services.get(&id) != Some(&definition.revision) {
                    return Err("服务或其依赖未授权，或配置已改变，需重新授权".into());
                }
            }
            Ok(())
        };
        let check_workspace = |id: &str| -> Result<(), String> {
            let group = self
                .workspaces()?
                .into_iter()
                .find(|w| w.id == id)
                .ok_or("工作区不存在")?;
            if grant.workspaces.get(id) != Some(&group.revision) {
                return Err("工作区未授权或配置已改变".into());
            }
            let definitions: Vec<_> = self.services()?.into_iter().map(|s| s.definition).collect();
            for id in ordered(&group.service_ids, &definitions)? {
                check_service(&id)?;
            }
            Ok(())
        };
        match method {
            "scan_start" => {
                let root = std::path::Path::new(args["root"].as_str().ok_or("缺少扫描目录")?)
                    .canonicalize()
                    .map_err(|_| "扫描目录不可访问")?;
                if !grant.scan_roots.iter().any(|r| root.starts_with(r)) {
                    return Err("扫描目录不在授权范围内".into());
                }
            }
            "scan_status" | "scan_cancel" | "prepare_import_projects" => {
                let scan = if method == "prepare_import_projects" {
                    args["scan_id"].as_str().unwrap_or("")
                } else {
                    id
                };
                if self.agent_scans.lock().map_err(|_| "扫描锁异常")?.get(scan) != Some(&grant.id)
                {
                    return Err("扫描会话不属于当前授权".into());
                }
            }
            "import_projects" => {
                let token = args["token"].as_str().ok_or("导入需要先预览并确认")?;
                let mut pending = self.agent_confirmations.lock().map_err(|_| "确认锁异常")?;
                let (owner, target, kind, fingerprint) =
                    pending.get(token).cloned().ok_or("导入确认已失效")?;
                let requested =
                    serde_json::to_string(&serde_json::json!([args["scan_id"], args["ids"]]))
                        .map_err(|_| "导入参数无效")?;
                let parts: Vec<_> = fingerprint.splitn(2, ':').collect();
                let issued = parts
                    .first()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                if owner != grant.id
                    || kind != "import_projects"
                    || args["scan_id"] != target
                    || parts.get(1).copied() != Some(digest(&requested).as_str())
                    || now().saturating_sub(issued) > 60
                {
                    return Err("导入确认对象、候选或有效期不符".into());
                }
                pending.remove(token);
            }
            "prepare_service_action" => {
                if !["start", "restart"].contains(&args["action"].as_str().unwrap_or_default()) {
                    return Err("MCP 不提供强制终止".into());
                }
                check_service(id)?;
            }
            "stop_service" => {
                check_service(id)?;
            }
            "prepare_workspace_action" => {
                check_workspace(id)?;
            }
            "confirm_service_action" | "confirm_workspace_action" => {
                let token = args["token"].as_str().ok_or("缺少确认令牌")?;
                let mut pending = self.agent_confirmations.lock().map_err(|_| "确认锁异常")?;
                let (owner, target, kind, action) = pending
                    .get(token)
                    .cloned()
                    .ok_or("MCP 确认令牌不存在或不属于当前客户端")?;
                if owner != grant.id
                    || kind != method
                    || args["id"] != target
                    || args["action"] != action
                {
                    return Err("确认令牌不属于当前授权、对象或操作".into());
                }
                pending.remove(token);
                if method == "confirm_service_action" {
                    check_service(&target)?;
                } else {
                    check_workspace(&target)?;
                }
            }
            "open_project" if grant.project_ids.iter().any(|p| p == id) => {
                if !["DEFAULT", "IDEA", "VSCODE", "CURSOR", "FINDER", "TERMINAL"]
                    .contains(&args["target"].as_str().unwrap_or_default())
                {
                    return Err("MCP 只能打开项目默认应用或既有固定目标".into());
                }
            }
            _ => return Err("MCP 无权调用此操作".into()),
        }
        Ok(grant.id)
    }
    pub(crate) fn bind_agent_confirmation(
        &self,
        context: &RequestContext,
        client: &str,
        method: &str,
        args: &Value,
        result: &Value,
    ) -> Result<(), String> {
        if context.origin == "MCP" && method == "scan_start" {
            let mut scans = self.agent_scans.lock().map_err(|_| "扫描锁异常")?;
            scans.clear();
            scans.insert(result.as_str().ok_or("扫描响应无效")?.into(), client.into());
        }
        if context.origin == "MCP" && method == "prepare_import_projects" {
            let requested =
                serde_json::to_string(&serde_json::json!([args["scan_id"], args["ids"]]))
                    .map_err(|_| "导入参数无效")?;
            let mut pending = self.agent_confirmations.lock().map_err(|_| "确认锁异常")?;
            if pending.len() >= 4096 {
                pending.clear();
            }
            pending.insert(
                result["token"].as_str().ok_or("导入确认无效")?.into(),
                (
                    client.into(),
                    args["scan_id"].as_str().unwrap_or_default().into(),
                    "import_projects".into(),
                    format!("{}:{}", now(), digest(&requested)),
                ),
            );
        }
        if context.origin == "MCP"
            && ["prepare_service_action", "prepare_workspace_action"].contains(&method)
        {
            let token = result["token"].as_str().ok_or("确认响应无效")?;
            let kind = if method == "prepare_service_action" {
                "confirm_service_action"
            } else {
                "confirm_workspace_action"
            };
            let mut pending = self.agent_confirmations.lock().map_err(|_| "确认锁异常")?;
            // Expiry is additionally enforced by each action's 60-second confirmation.
            if pending.len() >= 4096 {
                pending.clear();
            }
            pending.insert(
                token.into(),
                (
                    client.into(),
                    args["id"].as_str().unwrap_or_default().into(),
                    kind.into(),
                    args["action"].as_str().unwrap_or_default().into(),
                ),
            );
        }
        Ok(())
    }
    pub(crate) fn record_access(
        &self,
        context: &RequestContext,
        client: &str,
        method: &str,
        args: &Value,
        success: bool,
    ) -> Result<(), String> {
        let subject = args["id"]
            .as_str()
            .filter(|s| Uuid::parse_str(s).is_ok())
            .unwrap_or("");
        let db = self.db.lock().map_err(|_| "数据库锁异常")?;
        // Do not persist request arguments, tokens, environment, command text or raw errors.
        db.execute("INSERT INTO access_audits(origin,client_id,action,subject,result,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![context.origin,client,method,subject,if success {"OK"} else {"DENIED_OR_FAILED"},now()]).map_err(|e| e.to_string())?;
        crate::storage::audit(
            &db,
            &format!("{}:{method}", context.origin),
            subject,
            if success { "OK" } else { "DENIED_OR_FAILED" },
        )
    }
    pub fn access_audits(&self) -> Result<Vec<AccessAudit>, String> {
        let db = self.db.lock().map_err(|_| "数据库锁异常")?;
        let mut statement = db.prepare("SELECT origin,client_id,action,subject,result,created_at FROM access_audits ORDER BY seq DESC LIMIT 100").map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |r| {
                Ok(AccessAudit {
                    origin: r.get(0)?,
                    client_id: r.get(1)?,
                    action: r.get(2)?,
                    subject: r.get(3)?,
                    result: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.map(|r| r.map_err(|e| e.to_string())).collect()
    }
}
