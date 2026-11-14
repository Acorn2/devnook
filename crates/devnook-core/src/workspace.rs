use crate::{
    model::now, service_model::*, service_storage as services, supervisor::reconcile, Core,
};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceGroup {
    pub id: String,
    pub name: String,
    pub description: String,
    pub service_ids: Vec<String>,
    pub revision: String,
    pub created_at: u64,
    pub updated_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRun {
    pub id: String,
    pub workspace_id: String,
    pub revision: String,
    pub state: String,
    pub plan: Vec<ServiceView>,
    pub owned_run_ids: Vec<String>,
    pub reused_service_ids: Vec<String>,
    pub cancel_requested: bool,
    pub error: Option<String>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}
impl WorkspaceRun {
    pub fn busy(&self) -> bool {
        matches!(
            self.state.as_str(),
            "STARTING" | "ROLLING_BACK" | "STOPPING"
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePreview {
    pub token: String,
    pub action: String,
    pub workspace: WorkspaceGroup,
    pub plan: Vec<ServiceView>,
    pub previous_run_id: Option<String>,
    pub owned_run_ids: Vec<String>,
    pub reused_service_ids: Vec<String>,
    pub issued_at: u64,
    pub expires_in_seconds: u64,
}
fn rows<T: DeserializeOwned>(db: &Connection, sql: &str, id: &str) -> Result<Vec<T>, String> {
    let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
    let values = stmt
        .query_map([id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    values
        .map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
fn save_run(db: &Connection, run: &WorkspaceRun) -> Result<(), String> {
    db.execute("INSERT INTO workspace_runs(id,workspace_id,state,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET state=excluded.state,payload=excluded.payload",
        params![run.id,run.workspace_id,run.state,serde_json::to_string(run).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn ordered(ids: &[String], definitions: &[ServiceDefinition]) -> Result<Vec<String>, String> {
    let map: HashMap<_, _> = definitions.iter().map(|d| (d.id.as_str(), d)).collect();
    fn visit(
        id: &str,
        map: &HashMap<&str, &ServiceDefinition>,
        visiting: &mut HashSet<String>,
        done: &mut HashSet<String>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        if done.contains(id) {
            return Ok(());
        }
        if visiting.len() >= 128 || out.len() >= 128 {
            return Err("工作区最多展开 128 个服务".into());
        }
        if !visiting.insert(id.into()) {
            return Err(format!("服务依赖形成循环：{id}"));
        }
        let definition = map
            .get(id)
            .ok_or_else(|| format!("依赖服务不存在或已归档：{id}"))?;
        let mut unique = HashSet::new();
        for dependency in &definition.dependencies {
            if !unique.insert(dependency) {
                return Err("依赖服务不能重复".into());
            }
            visit(dependency, map, visiting, done, out)?;
        }
        visiting.remove(id);
        done.insert(id.into());
        out.push(id.into());
        Ok(())
    }
    let mut out = Vec::new();
    let mut visiting = HashSet::new();
    let mut done = HashSet::new();
    for id in ids {
        visit(id, &map, &mut visiting, &mut done, &mut out)?;
    }
    Ok(out)
}
impl Core {
    pub fn workspaces(&self) -> Result<Vec<WorkspaceGroup>, String> {
        rows(
            &*self.db.lock().map_err(|_| "数据库锁异常")?,
            "SELECT payload FROM workspace_groups WHERE ?1='' ORDER BY rowid",
            "",
        )
    }
    pub fn workspace_history(&self, id: &str) -> Result<Vec<WorkspaceRun>, String> {
        rows(
            &*self.db.lock().map_err(|_| "数据库锁异常")?,
            "SELECT payload FROM workspace_runs WHERE workspace_id=?1 ORDER BY seq DESC LIMIT 50",
            id,
        )
    }
    pub fn workspace_jobs(&self) -> Result<Vec<WorkspaceRun>, String> {
        rows(&*self.db.lock().map_err(|_| "数据库锁异常")?, "SELECT payload FROM workspace_runs WHERE state IN ('STARTING','ROLLING_BACK','STOPPING') AND ?1='' ORDER BY seq", "")
    }
    pub(crate) fn ensure_service_available(&self, id: &str) -> Result<(), String> {
        if self
            .workspace_jobs()?
            .iter()
            .any(|r| r.plan.iter().any(|s| s.definition.id == id))
        {
            return Err("服务正在参与工作区操作，请等待结束或停止该工作区".into());
        }
        Ok(())
    }
    pub(crate) fn ensure_service_unreferenced(&self, id: &str) -> Result<(), String> {
        if self
            .services()?
            .iter()
            .any(|s| s.definition.dependencies.iter().any(|d| d == id))
            || self
                .workspaces()?
                .iter()
                .any(|w| w.service_ids.iter().any(|s| s == id))
        {
            return Err("服务仍被依赖或工作区引用，请先移除引用".into());
        }
        Ok(())
    }
    pub(crate) fn validate_dependencies(
        &self,
        definition: &ServiceDefinition,
    ) -> Result<(), String> {
        let mut definitions: Vec<_> = self
            .services()?
            .into_iter()
            .map(|s| s.definition)
            .filter(|d| d.id != definition.id)
            .collect();
        definitions.push(definition.clone());
        ordered(&[definition.id.clone()], &definitions)?;
        Ok(())
    }
    pub fn save_workspace(&self, mut group: WorkspaceGroup) -> Result<WorkspaceGroup, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "操作锁异常")?;
        if group.name.trim().is_empty()
            || group.name.len() > 160
            || group.description.len() > 2048
            || group.service_ids.is_empty()
            || group.service_ids.len() > 64
        {
            return Err("工作区名称必填，需选择 1–64 个服务".into());
        }
        if group.service_ids.iter().collect::<HashSet<_>>().len() != group.service_ids.len() {
            return Err("工作区条目不能重复".into());
        }
        ordered(
            &group.service_ids,
            &self
                .services()?
                .into_iter()
                .map(|s| s.definition)
                .collect::<Vec<_>>(),
        )?;
        if group.id.is_empty() {
            group.id = Uuid::new_v4().to_string();
            group.created_at = now();
        } else {
            let previous = self
                .workspaces()?
                .into_iter()
                .find(|w| w.id == group.id)
                .ok_or("工作区不存在")?;
            if previous.revision != group.revision {
                return Err("工作区已更新，请重新编辑".into());
            }
            if self
                .workspace_history(&group.id)?
                .first()
                .is_some_and(|r| r.busy() || r.state == "READY" || r.state == "ROLLBACK_FAILED")
            {
                return Err("请先停止工作区，再修改配置".into());
            }
            group.created_at = previous.created_at;
        }
        group.name = group.name.trim().into();
        group.updated_at = now();
        group.revision = Uuid::new_v4().to_string();
        self.db.lock().map_err(|_| "数据库锁异常")?.execute("INSERT INTO workspace_groups(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            params![group.id,serde_json::to_string(&group).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        Ok(group)
    }
    pub fn remove_workspace(&self, id: &str) -> Result<(), String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "操作锁异常")?;
        if self
            .workspace_history(id)?
            .first()
            .is_some_and(|r| r.busy() || r.state == "READY" || r.state == "ROLLBACK_FAILED")
        {
            return Err("请先停止工作区".into());
        }
        self.db
            .lock()
            .map_err(|_| "数据库锁异常")?
            .execute("DELETE FROM workspace_groups WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn prepare_workspace_action(
        &self,
        id: &str,
        action: &str,
    ) -> Result<WorkspacePreview, String> {
        let workspace = self
            .workspaces()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or("工作区不存在")?;
        let previous = self.workspace_history(id)?.into_iter().next();
        if action == "start" {
            if previous
                .as_ref()
                .is_some_and(|r| r.busy() || r.state == "READY" || r.state == "ROLLBACK_FAILED")
            {
                return Err("工作区已有活动记录，请先停止".into());
            }
        } else if action != "stop" || previous.is_none() {
            return Err("无可停止的工作区记录".into());
        }
        let views = self.services()?;
        let definitions: Vec<_> = views.iter().map(|s| s.definition.clone()).collect();
        let ids = if action == "start" {
            ordered(&workspace.service_ids, &definitions)?
        } else {
            previous
                .as_ref()
                .unwrap()
                .plan
                .iter()
                .map(|v| v.definition.id.clone())
                .collect()
        };
        let plan = ids
            .iter()
            .map(|id| {
                views
                    .iter()
                    .find(|v| &v.definition.id == id)
                    .cloned()
                    .ok_or_else(|| format!("服务 {id} 已归档"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let preview = WorkspacePreview {
            token: Uuid::new_v4().to_string(),
            action: action.into(),
            workspace,
            plan,
            owned_run_ids: previous
                .as_ref()
                .map(|r| r.owned_run_ids.clone())
                .unwrap_or_default(),
            reused_service_ids: previous
                .as_ref()
                .map(|r| r.reused_service_ids.clone())
                .unwrap_or_default(),
            previous_run_id: previous.map(|r| r.id),
            issued_at: now(),
            expires_in_seconds: 60,
        };
        let mut pending = self
            .workspace_confirmations
            .lock()
            .map_err(|_| "确认锁异常")?;
        pending.retain(|_, p| now().saturating_sub(p.issued_at) <= 60);
        pending.insert(preview.token.clone(), preview.clone());
        Ok(preview)
    }
    pub fn confirm_workspace_action(self: &Arc<Self>, token: &str) -> Result<WorkspaceRun, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "操作锁异常")?;
        let preview = self
            .workspace_confirmations
            .lock()
            .map_err(|_| "确认锁异常")?
            .remove(token)
            .ok_or("工作区确认已失效")?;
        if now().saturating_sub(preview.issued_at) > 60 {
            return Err("工作区确认已过期".into());
        }
        let current = self
            .workspaces()?
            .into_iter()
            .find(|w| w.id == preview.workspace.id)
            .ok_or("工作区不存在")?;
        if current.revision != preview.workspace.revision {
            return Err("工作区配置已改变".into());
        }
        let previous = self.workspace_history(&current.id)?.into_iter().next();
        if previous.as_ref().map(|r| &r.id) != preview.previous_run_id.as_ref() {
            return Err("工作区运行已改变，请重新预览".into());
        }
        for planned in &preview.plan {
            let current = self
                .services()?
                .into_iter()
                .find(|s| s.definition.id == planned.definition.id)
                .ok_or("服务已归档")?;
            if current.definition.revision != planned.definition.revision
                || current.current_run.as_ref().map(|r| &r.id)
                    != planned.current_run.as_ref().map(|r| &r.id)
            {
                return Err("服务配置或运行已改变，请重新预览".into());
            }
        }
        if preview.action == "stop" {
            let mut run = previous.ok_or("运行记录不存在")?;
            if run.busy() {
                run.cancel_requested = true;
                save_run(&*self.db.lock().map_err(|_| "数据库锁异常")?, &run)?;
                return Ok(run);
            }
            for service in &run.plan {
                self.ensure_service_available(&service.definition.id)?;
            }
            run.state = "STOPPING".into();
            save_run(&*self.db.lock().map_err(|_| "数据库锁异常")?, &run)?;
            let core = self.clone();
            let result = run.clone();
            std::thread::spawn(move || core.finish_workspace(run, true));
            return Ok(result);
        }
        for service in &preview.plan {
            self.ensure_service_available(&service.definition.id)?;
        }
        let run = WorkspaceRun {
            id: Uuid::new_v4().to_string(),
            workspace_id: current.id,
            revision: current.revision,
            state: "STARTING".into(),
            plan: preview.plan,
            owned_run_ids: vec![],
            reused_service_ids: vec![],
            cancel_requested: false,
            error: None,
            started_at: now(),
            finished_at: None,
        };
        save_run(&*self.db.lock().map_err(|_| "数据库锁异常")?, &run)?;
        let core = self.clone();
        let result = run.clone();
        std::thread::spawn(move || core.launch_workspace(run));
        Ok(result)
    }
    fn cancelled(&self, run: &WorkspaceRun) -> Result<bool, String> {
        Ok(self
            .workspace_history(&run.workspace_id)?
            .into_iter()
            .find(|r| r.id == run.id)
            .ok_or("工作区记录不存在")?
            .cancel_requested)
    }
    fn persist_workspace(&self, run: &mut WorkspaceRun) -> Result<(), String> {
        // Serialize with cancellation to avoid replacing a just-written cancel request.
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "操作锁异常")?;
        run.cancel_requested |= self.cancelled(run)?;
        save_run(&*self.db.lock().map_err(|_| "数据库锁异常")?, run)
    }
    fn launch_workspace(self: Arc<Self>, mut run: WorkspaceRun) {
        let result = (|| -> Result<(), String> {
            for view in run.plan.clone() {
                if self.cancelled(&run)? {
                    return Err("用户已取消启动".into());
                }
                let current = {
                    let _operation = self
                        .supervisor
                        .operations
                        .lock()
                        .map_err(|_| "操作锁异常")?;
                    if self.cancelled(&run)? {
                        return Err("用户已取消启动".into());
                    }
                    let current = self
                        .service_history(&view.definition.id)?
                        .into_iter()
                        .next();
                    if let Some(active) = current.filter(|r| r.state.active()) {
                        run.reused_service_ids.push(view.definition.id.clone());
                        active
                    } else {
                        self.start_run(view.definition.clone(), Some(run.id.clone()))?
                    }
                };
                if current.workspace_run_id.as_deref() == Some(&run.id) {
                    run.owned_run_ids.push(current.id.clone());
                }
                self.persist_workspace(&mut run)?;
                let deadline = Instant::now()
                    + Duration::from_secs(view.definition.readiness.timeout_seconds + 5);
                loop {
                    if self.cancelled(&run)? {
                        return Err("用户已取消启动".into());
                    }
                    let current = reconcile(services::run(
                        &*self.db.lock().map_err(|_| "数据库锁异常")?,
                        &current.id,
                    )?);
                    if current.state == ServiceState::Ready {
                        break;
                    }
                    if !current.state.active()
                        || matches!(
                            current.state,
                            ServiceState::Stopping | ServiceState::Unknown
                        )
                    {
                        return Err(format!(
                            "{} 未就绪：{}",
                            view.definition.name,
                            current
                                .error
                                .unwrap_or_else(|| format!("{:?}", current.state))
                        ));
                    }
                    if Instant::now() > deadline {
                        return Err(format!("{} 等待 READY 超时", view.definition.name));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            // Earlier services may have failed while a later dependency was starting.
            for view in &run.plan {
                if !self
                    .service_history(&view.definition.id)?
                    .first()
                    .is_some_and(|r| r.state == ServiceState::Ready)
                {
                    return Err(format!("{} 已失去就绪状态", view.definition.name));
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            run.error = Some(error);
            self.finish_workspace(run, false);
        } else {
            run.state = "READY".into();
            // A cancel arriving at the final boundary must still roll back.
            if self.persist_workspace(&mut run).is_ok() && run.cancel_requested {
                self.finish_workspace(run, true);
            }
        }
    }
    fn finish_workspace(&self, mut run: WorkspaceRun, stopping: bool) {
        run.state = if stopping { "STOPPING" } else { "ROLLING_BACK" }.into();
        let result = (|| -> Result<(), String> {
            self.persist_workspace(&mut run)?;
            // Recover ownership even if Core died after saving a service run but before journaling it.
            let owned: Vec<ServiceRun> = rows(&*self.db.lock().map_err(|_| "数据库锁异常")?,
                "SELECT payload FROM service_runs WHERE json_extract(payload,'$.workspace_run_id')=?1 ORDER BY seq DESC", &run.id)?;
            run.owned_run_ids = owned.iter().rev().map(|r| r.id.clone()).collect();
            self.persist_workspace(&mut run)?;
            let mut failures = Vec::new();
            for saved in owned {
                let current = reconcile(services::run(
                    &*self.db.lock().map_err(|_| "数据库锁异常")?,
                    &saved.id,
                )?);
                if !current.state.active() {
                    continue;
                }
                let grace = run
                    .plan
                    .iter()
                    .find(|v| v.definition.id == current.service_id)
                    .map(|v| v.definition.stop_grace_seconds)
                    .unwrap_or(5);
                let stopped = (|| -> Result<(), String> {
                    let _operation = self
                        .supervisor
                        .operations
                        .lock()
                        .map_err(|_| "操作锁异常")?;
                    self.stop_run(&current)
                })();
                if let Err(error) = stopped {
                    failures.push(format!("{}: {error}", current.service_id));
                    continue;
                }
                let deadline = Instant::now() + Duration::from_secs(grace + 2);
                loop {
                    let current = reconcile(services::run(
                        &*self.db.lock().map_err(|_| "数据库锁异常")?,
                        &saved.id,
                    )?);
                    if !current.state.active() {
                        break;
                    }
                    if Instant::now() > deadline {
                        failures.push(format!("{} 停止未完成，需人工检查", saved.service_id));
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(failures.join("；"))
            }
        })();
        run.state = if let Err(error) = result {
            run.error = Some(format!(
                "{}；回滚/停止：{error}",
                run.error.unwrap_or_default()
            ));
            "ROLLBACK_FAILED"
        } else if stopping {
            "STOPPED"
        } else {
            "FAILED"
        }
        .into();
        run.finished_at = Some(now());
        let _ = self.persist_workspace(&mut run);
    }
    pub fn recover_workspaces(self: &Arc<Self>) -> Result<(), String> {
        self.recover_workspace_runs(self.workspace_jobs()?)
    }
    pub(crate) fn recover_workspace_runs(
        &self,
        interrupted: Vec<WorkspaceRun>,
    ) -> Result<(), String> {
        for mut run in interrupted {
            run.error = Some("Core 上次中断，已停止继续启动并回滚本次新增实例".into());
            self.finish_workspace(run, false);
        }
        Ok(())
    }
}
