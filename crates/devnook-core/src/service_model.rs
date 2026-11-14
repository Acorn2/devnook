use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Ready,
    Stopping,
    Blocked,
    Crashed,
    Unknown,
}
impl ServiceState {
    pub fn active(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running | Self::Ready | Self::Stopping | Self::Unknown
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessKind {
    Process,
    Port,
    Http,
    LogPhrase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Readiness {
    pub kind: ReadinessKind,
    pub port: Option<u16>,
    pub url: String,
    pub phrase: String,
    pub timeout_seconds: u64,
    pub interval_ms: u64,
    pub grace_ms: u64,
}
impl Default for Readiness {
    fn default() -> Self {
        Self {
            kind: ReadinessKind::Process,
            port: None,
            url: String::new(),
            phrase: String::new(),
            timeout_seconds: 30,
            interval_ms: 500,
            grace_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub runtime_type: String,
    pub executable: String,
    pub args: Vec<String>,
    pub shell_mode: bool,
    pub working_directory: String,
    pub environment: BTreeMap<String, String>,
    pub secret_keys: Vec<String>,
    pub expected_ports: Vec<u16>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub readiness: Readiness,
    pub stop_grace_seconds: u64,
    pub log_limit_kb: u64,
    pub revision: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ServiceSave {
    pub definition: ServiceDefinition,
    #[serde(default)]
    pub secret_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRun {
    pub id: String,
    pub service_id: String,
    pub project_id: String,
    pub revision: String,
    pub launch_source: String,
    #[serde(default)]
    pub workspace_run_id: Option<String>,
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub worker_pid: Option<u32>,
    pub pgid: Option<u32>,
    pub worker_start: Option<String>,
    pub started_at: u64,
    pub stopped_at: Option<u64>,
    pub ready_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub log_path: String,
    pub stop_requested: bool,
    pub stop_requested_at_ms: Option<u64>,
    pub log_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceView {
    pub definition: ServiceDefinition,
    pub current_run: Option<ServiceRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceActionPreview {
    pub token: String,
    pub action: String,
    pub service: ServiceDefinition,
    pub run: Option<ServiceRun>,
    pub command: String,
    pub environment_keys: Vec<String>,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPage {
    pub run_id: String,
    pub lines: Vec<String>,
    pub truncated: bool,
}
