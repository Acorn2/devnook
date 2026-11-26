use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentCandidate {
    pub name: String,
    pub canonical_path: String,
    pub project_type: String,
    pub frameworks: Vec<String>,
    pub build_tools: Vec<String>,
    pub evidence: Vec<String>,
    pub runnable: bool,
    pub needs_review: bool,
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub canonical_path: String,
    pub project_type: String,
    pub frameworks: Vec<String>,
    pub build_tools: Vec<String>,
    pub evidence: Vec<String>,
    pub is_git: bool,
    pub is_monorepo: bool,
    pub parent_path: Option<String>,
    pub runnable: bool,
    pub needs_review: bool,
    pub imported: bool,
    #[serde(default)]
    pub components: Vec<ComponentCandidate>,
    #[serde(default)]
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub candidate: Candidate,
    pub default_ide: String,
    pub is_favorite: bool,
    pub category: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEdit {
    pub id: String,
    pub name: String,
    pub default_ide: String,
    pub is_favorite: bool,
    pub category: String,
}

fn default_terminal() -> String {
    "TERMINAL".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub scan_depth: usize,
    pub refresh_seconds: u64,
    pub default_ide: String,
    #[serde(default = "default_terminal")]
    pub default_terminal: String,
    pub idea_path: String,
    pub vscode_path: String,
    pub cursor_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scan_depth: 4,
            refresh_seconds: 2,
            default_ide: "VSCODE".into(),
            default_terminal: default_terminal(),
            idea_path: String::new(),
            vscode_path: String::new(),
            cursor_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanProgress {
    pub id: String,
    pub status: String,
    pub visited: usize,
    #[serde(default)]
    pub entries: usize,
    pub current_path: String,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub incomplete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortBinding {
    pub pid: u32,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub family: String,
    pub first_seen: u64,
    pub project_id: Option<String>,
    pub command: String,
    pub cwd: Option<String>,
    #[serde(default = "observed")]
    pub source: String,
    #[serde(default)]
    pub service_id: Option<String>,
    /// Display classification only; never grants ownership or process control.
    #[serde(default)]
    pub process_kind: ProcessKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessKind {
    Service,
    DevelopmentTool,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentProcess {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetail {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub uid: u32,
    pub name: String,
    pub command: String,
    pub executable: Option<String>,
    pub cwd: Option<String>,
    pub start_time: String,
    pub elapsed: String,
    pub fingerprint: String,
    pub start_identity: Option<String>,
    pub bindings: Vec<PortBinding>,
    pub parents: Vec<ParentProcess>,
    pub project_id: Option<String>,
    pub suggested_project_id: Option<String>,
    pub confidence: String,
    pub source: String,
    pub can_stop: bool,
    pub stop_reason: String,
    #[serde(default)]
    pub service_id: Option<String>,
    #[serde(default)]
    pub memory_rss_bytes: Option<u64>,
    #[serde(default)]
    pub cpu_time_ms: Option<u64>,
}
fn observed() -> String {
    "OBSERVED".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopPreview {
    pub token: String,
    pub process: ProcessDetail,
    pub expires_in_seconds: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
