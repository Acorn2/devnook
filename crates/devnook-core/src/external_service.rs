//! Explicit external-to-managed replacement. Saving a draft never adopts a PID.
use crate::{
    model::{now, ProcessDetail, ProcessKind, Project, StopPreview},
    process,
    service_drafts::ServiceDraft,
    service_model::*,
    supervisor, workspace, Core,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServiceConfiguration {
    pub process: ProcessDetail,
    pub draft: ServiceDraft,
    /// Whether the connected daemon supports the separate, confirmed handoff.
    #[serde(default = "handoff_supported")]
    pub handoff_supported: bool,
}

fn handoff_supported() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expired_handoff_is_consumed_before_any_process_or_service_access() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::new(&temp.path().join("registry.db")).unwrap();
        let process: ProcessDetail = serde_json::from_value(json!({
            "pid":1,"ppid":0,"pgid":1,"uid":0,"name":"expired fixture","command":"",
            "executable":null,"cwd":null,"start_time":"","elapsed":"","fingerprint":"expired",
            "start_identity":null,"bindings":[],"parents":[],"project_id":null,"suggested_project_id":null,
            "confidence":"UNKNOWN","source":"OBSERVED","can_stop":false,"stop_reason":""
        })).unwrap();
        let service: ServiceDefinition = serde_json::from_value(json!({
            "id":"not-a-real-service","project_id":"none","name":"expired","runtime_type":"OTHER",
            "executable":"","args":[],"shell_mode":false,"working_directory":"","environment":{},
            "secret_keys":[],"expected_ports":[],"readiness":Readiness::default(),"stop_grace_seconds":1,
            "log_limit_kb":64,"revision":"expired","created_at":0,"updated_at":0
        })).unwrap();
        core.external_handoffs.lock().unwrap().insert(
            "expired".into(),
            PendingHandoff {
                preview: ExternalServiceHandoffPreview {
                    token: "expired".into(),
                    process,
                    service,
                    command: String::new(),
                    environment_keys: vec![],
                    dependencies: vec![],
                    expires_in_seconds: 60,
                },
                issued: 0,
                run_id: None,
            },
        );
        let error = core
            .confirm_external_service_handoff("expired", 1)
            .unwrap_err();
        assert!(error.contains("已过期"));
        assert!(core
            .confirm_external_service_handoff("expired", 1)
            .unwrap_err()
            .contains("已失效"));
        assert!(core.services().unwrap().is_empty());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServiceHandoffPreview {
    pub token: String,
    pub process: ProcessDetail,
    pub service: ServiceDefinition,
    pub command: String,
    pub environment_keys: Vec<String>,
    pub dependencies: Vec<ServiceView>,
    pub expires_in_seconds: u64,
}

pub(crate) struct PendingHandoff {
    preview: ExternalServiceHandoffPreview,
    run_id: Option<String>,
    issued: u64,
}

fn ports(process: &ProcessDetail) -> BTreeSet<u16> {
    process.bindings.iter().map(|b| b.port).collect()
}

// A missing identity is not proof of exit: distinguish ESRCH from read failures.
fn exited(previous: &ProcessDetail) -> Result<bool, String> {
    if let Some(current) = process::identity(previous.pid) {
        if Some(&current.start) != previous.start_identity.as_ref() {
            return Err("旧 PID 的进程身份已变化，已中止切换".into());
        }
        return Ok(!current.alive);
    }
    // macOS may no longer return proc_bsdinfo for a zombie. A positive Z state
    // proves execution ended even when the original parent has not reaped it.
    if process::command("/bin/ps", &["-p", &previous.pid.to_string(), "-o", "stat="])
        .is_ok_and(|state| state.trim().starts_with('Z'))
    {
        return Ok(true);
    }
    let result = unsafe { libc::kill(previous.pid as i32, 0) };
    if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        Ok(true)
    } else if result == 0 {
        // It may be between exit and reaping, or temporarily unreadable. Wait,
        // but never treat that uncertainty as permission to start a replacement.
        Ok(false)
    } else {
        Err("无法核验旧进程是否退出，已中止切换".into())
    }
}

// Shared with the local client compatibility path; an old daemon must not relax
// project, cwd, or service-purpose checks just because it lacks the newer RPC.
pub(crate) fn validate_external_service(
    detail: &ProcessDetail,
    projects: &[Project],
) -> Result<(), String> {
    if detail.source != "OBSERVED"
        || !detail
            .bindings
            .iter()
            .any(|b| b.process_kind == ProcessKind::Service)
    {
        return Err("当前进程不是可配置的外部项目服务，请刷新后重试".into());
    }
    let project = projects
        .iter()
        .find(|p| Some(&p.candidate.id) == detail.project_id.as_ref())
        .ok_or("请先确认外部服务的项目归属")?;
    let cwd = detail.cwd.as_deref().ok_or("无法核验外部服务工作目录")?;
    if !Path::new(cwd).starts_with(&project.candidate.canonical_path)
        || !["HIGH", "MANUAL"].contains(&detail.confidence.as_str())
    {
        return Err("外部服务工作目录与已确认项目不符".into());
    }
    Ok(())
}

pub(crate) fn configuration(
    process: ProcessDetail,
    drafts: Vec<ServiceDraft>,
    handoff_supported: bool,
    project_root: &Path,
) -> Result<ExternalServiceConfiguration, String> {
    let project_id = process.project_id.as_deref().ok_or("项目归属不存在")?;
    let cwd = process.cwd.clone().ok_or("工作目录不可用")?;
    let mut matching = drafts
        .into_iter()
        .filter(|d| d.definition.working_directory == cwd);
    let first = matching.next();
    let mut draft = match (first, matching.next()) {
        (Some(draft), None) => draft,
        _ => ServiceDraft {
            definition: ServiceDefinition {
                id: String::new(),
                project_id: project_id.into(),
                name: process.name.clone(),
                runtime_type: "OTHER".into(),
                executable: String::new(),
                args: vec![],
                shell_mode: false,
                working_directory: cwd,
                environment: Default::default(),
                secret_keys: vec![],
                expected_ports: vec![],
                dependencies: vec![],
                readiness: Readiness::default(),
                stop_grace_seconds: 5,
                log_limit_kb: 1024,
                revision: String::new(),
                created_at: 0,
                updated_at: 0,
            },
            command: String::new(),
            source: "无法从当前工作目录唯一识别启动方式，请填写可重复执行的命令".into(),
        },
    };
    if let Some((spec, name, source)) =
        crate::service_drafts::external_node_command(&process, project_root)
    {
        draft.definition.name = name;
        draft.definition.runtime_type = "NODE".into();
        draft.definition.executable = spec.executable.clone();
        draft.definition.args = spec.args.clone();
        draft.definition.shell_mode = false;
        draft.command = crate::service_drafts::display_command(&spec);
        draft.source = source;
    }
    draft.definition.expected_ports = ports(&process).into_iter().collect();
    // Only a manifest-backed suggestion; never copy ps output, env or IDE flags.
    draft
        .source
        .push_str("；预期端口来自本次外部监听，保存前请核对");
    Ok(ExternalServiceConfiguration {
        process,
        draft,
        handoff_supported,
    })
}

impl Core {
    fn external_service(&self, pid: u32) -> Result<ProcessDetail, String> {
        let detail = self.inspect(pid)?;
        validate_external_service(&detail, &self.projects()?)?;
        Ok(detail)
    }

    pub fn prepare_external_service_stop(&self, pid: u32) -> Result<StopPreview, String> {
        let process = self.external_service(pid)?;
        let preview = self.prepare_stop(pid)?;
        if preview.process.fingerprint != process.fingerprint
            || preview.process.project_id != process.project_id
        {
            self.stops
                .lock()
                .map_err(|_| "确认锁异常")?
                .remove(&preview.token);
            return Err("外部服务身份已变化，请重新预览".into());
        }
        Ok(preview)
    }

    pub fn external_service_configuration(
        &self,
        pid: u32,
    ) -> Result<ExternalServiceConfiguration, String> {
        let process = self.external_service(pid)?;
        let drafts = self.service_drafts(process.project_id.as_deref().ok_or("项目归属不存在")?)?;
        let project = self
            .projects()?
            .into_iter()
            .find(|p| Some(&p.candidate.id) == process.project_id.as_ref())
            .ok_or("项目归属不存在")?;
        configuration(
            process,
            drafts,
            true,
            Path::new(&project.candidate.canonical_path),
        )
    }

    pub fn prepare_external_service_handoff(
        &self,
        id: &str,
        revision: &str,
        pid: u32,
        fingerprint: &str,
    ) -> Result<ExternalServiceHandoffPreview, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        let process = self.external_service(pid)?;
        if process.fingerprint != fingerprint {
            return Err("原外部实例已变化；配置已保存，请刷新后重新操作".into());
        }
        process::stop_eligibility(&process, &self.protected)?;
        let start = self.prepare_service_action(id, "start")?;
        // Move the ordinary start confirmation into a separate, bound handoff token.
        self.supervisor
            .confirmations
            .lock()
            .map_err(|_| "确认锁异常")?
            .remove(&start.token);
        if start.service.revision != revision
            || Some(&start.service.project_id) != process.project_id.as_ref()
        {
            return Err("保存的服务配置或项目归属已变化，请重新预览".into());
        }
        let views = self.services()?;
        let definitions: Vec<_> = views.iter().map(|v| v.definition.clone()).collect();
        let dependencies = workspace::ordered(&[id.into()], &definitions)?
            .into_iter()
            .filter(|dep| dep != id)
            .map(|dep| {
                views
                    .iter()
                    .find(|v| v.definition.id == dep)
                    .cloned()
                    .ok_or("依赖服务不存在".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let preview = ExternalServiceHandoffPreview {
            token: start.token,
            process,
            service: start.service,
            command: start.command,
            environment_keys: start.environment_keys,
            dependencies,
            expires_in_seconds: 60,
        };
        let pending = PendingHandoff {
            preview: preview.clone(),
            run_id: start.run.map(|r| r.id),
            issued: now(),
        };
        self.check_handoff_configuration(&pending)?;
        let mut confirmations = self
            .external_handoffs
            .lock()
            .map_err(|_| "切换确认锁异常")?;
        confirmations.retain(|_, p| now().saturating_sub(p.issued) <= 60);
        confirmations.insert(preview.token.clone(), pending);
        Ok(preview)
    }

    fn check_handoff_configuration(
        &self,
        pending: &PendingHandoff,
    ) -> Result<ServiceDefinition, String> {
        let preview = &pending.preview;
        let views = self.services()?;
        let view = views
            .iter()
            .find(|v| v.definition.id == preview.service.id)
            .ok_or("服务配置已移除")?;
        self.ensure_service_available(&view.definition.id)?;
        if view.definition.revision != preview.service.revision
            || view.current_run.as_ref().map(|r| &r.id) != pending.run_id.as_ref()
            || view.current_run.as_ref().is_some_and(|r| r.state.active())
        {
            return Err("服务配置或运行实例已变化，已中止切换，请重新预览".into());
        }
        for previous in &preview.dependencies {
            let current = views
                .iter()
                .find(|v| v.definition.id == previous.definition.id)
                .ok_or("依赖服务已移除")?;
            self.ensure_service_available(&current.definition.id)?;
            if current.definition.revision != previous.definition.revision
                || current.current_run.as_ref().map(|r| &r.id)
                    != previous.current_run.as_ref().map(|r| &r.id)
                || !current
                    .current_run
                    .as_ref()
                    .is_some_and(|r| r.state == ServiceState::Ready)
            {
                return Err("依赖配置、运行实例已变化或尚未就绪，已中止切换".into());
            }
        }
        supervisor::validate(view.definition.clone(), self)
    }

    pub fn confirm_external_service_handoff(
        &self,
        token: &str,
        confirmed_pid: u32,
    ) -> Result<ServiceRun, String> {
        let _operation = self
            .supervisor
            .operations
            .lock()
            .map_err(|_| "服务操作锁异常")?;
        let pending = self
            .external_handoffs
            .lock()
            .map_err(|_| "切换确认锁异常")?
            .remove(token)
            .ok_or("切换确认已失效，请重新预览")?;
        let preview = &pending.preview;
        if now().saturating_sub(pending.issued) > 60 || confirmed_pid != preview.process.pid {
            return Err("切换确认已过期或 PID 不匹配，未停止旧实例".into());
        }
        let definition = self.check_handoff_configuration(&pending)?;
        let current = self.external_service(confirmed_pid)?;
        if current.fingerprint != preview.process.fingerprint
            || current.project_id != preview.process.project_id
            || ports(&current) != ports(&preview.process)
        {
            return Err("外部服务身份、归属或监听已变化，未停止旧实例".into());
        }
        // Preflight without running commands, before disrupting the old instance.
        supervisor::resolve_program(if definition.shell_mode {
            "/bin/zsh"
        } else {
            &definition.executable
        })?;
        for key in &definition.secret_keys {
            self.supervisor
                .secrets
                .get(&format!("{}:{}:{key}", definition.id, definition.revision))?;
        }
        let watched: BTreeSet<_> = ports(&current)
            .into_iter()
            .chain(definition.expected_ports.iter().copied())
            .collect();
        if process::list_ports()?
            .iter()
            .any(|b| watched.contains(&b.port) && b.pid != current.pid)
            || self.services()?.iter().any(|v| {
                v.current_run.as_ref().is_some_and(|r| r.state.active())
                    && v.definition
                        .expected_ports
                        .iter()
                        .any(|p| watched.contains(p))
            })
        {
            return Err("相关端口被其他进程或托管运行占用，未停止旧实例".into());
        }
        let stop = self.prepare_external_service_stop(confirmed_pid)?;
        if stop.process.fingerprint != preview.process.fingerprint {
            return Err("旧实例身份已变化，未发送停止信号".into());
        }
        self.confirm_stop(&stop.token, confirmed_pid)?;
        let deadline = Instant::now() + Duration::from_secs(definition.stop_grace_seconds);
        loop {
            if exited(&preview.process)? {
                if process::list_ports()?
                    .iter()
                    .any(|b| watched.contains(&b.port))
                {
                    return Err("旧实例已退出，但相关端口仍被监听；未启动新实例，配置已保留".into());
                }
                // Recheck immediately before launching; start_run also checks ports again.
                self.check_handoff_configuration(&pending)?;
                return self.start_run(definition, None);
            }
            if Instant::now() >= deadline {
                return Err(
                    "旧实例未在正常停止宽限期内退出；未强杀、未启动新实例，配置已保留".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
