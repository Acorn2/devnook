export interface ComponentCandidate {
  name: string;
  canonical_path: string;
  project_type: string;
  frameworks: string[];
  build_tools: string[];
  evidence: string[];
  runnable: boolean;
  needs_review: boolean;
  imported: boolean;
}
export interface Candidate {
  id: string;
  name: string;
  root_path: string;
  canonical_path: string;
  project_type: string;
  frameworks: string[];
  build_tools: string[];
  evidence: string[];
  is_git: boolean;
  is_monorepo: boolean;
  parent_path: string | null;
  runnable: boolean;
  needs_review: boolean;
  imported: boolean;
  components: ComponentCandidate[];
  boundary: string;
}
export interface Project extends Candidate {
  default_ide: string;
  is_favorite: boolean;
  category: string;
  created_at: number;
  updated_at: number;
}
export interface Settings {
  scan_depth: number;
  refresh_seconds: number;
  default_ide: string;
  idea_path: string;
  vscode_path: string;
  cursor_path: string;
}
export interface DevelopmentApp {
  id: string;
  name: string;
  bundle_id: string;
  path: string;
  version: string;
  kind: 'EDITOR' | 'IDE' | 'TERMINAL';
  recommended_for: string[];
}
export interface AppPreference {
  id: string;
  name: string;
  available: boolean;
}
export interface DevelopmentApps {
  apps: DevelopmentApp[];
  project_defaults: Record<string, AppPreference>;
  global_default: AppPreference;
  warnings: string[];
}
export interface ScanProgress {
  id: string;
  status: 'scanning' | 'completed' | 'cancelled';
  visited: number;
  entries: number;
  current_path: string;
  candidates: Candidate[];
  warnings: string[];
  incomplete: boolean;
}
export interface PortBinding {
  pid: number;
  name: string;
  address: string;
  port: number;
  family: string;
  first_seen: number;
  project_id: string | null;
  command: string;
  cwd: string | null;
  source: 'OBSERVED' | 'DEVNOOK';
  service_id: string | null;
  process_kind: 'SERVICE' | 'DEVELOPMENT_TOOL' | 'UNKNOWN';
}
export type BrowserTarget =
  { kind: 'MANAGED'; service_id: string } | { kind: 'EXTERNAL'; pid: number };
export interface BrowserAddress {
  url: string;
  pid: number;
  fingerprint: string;
  web_ui?: boolean | null;
}
export interface ProcessDetail {
  pid: number;
  ppid: number;
  pgid: number;
  uid: number;
  name: string;
  command: string;
  executable: string | null;
  cwd: string | null;
  start_time: string;
  elapsed: string;
  fingerprint: string;
  bindings: PortBinding[];
  parents: { pid: number; name: string }[];
  project_id: string | null;
  suggested_project_id: string | null;
  confidence: string;
  source: string;
  can_stop: boolean;
  stop_reason: string;
  service_id: string | null;
}
export interface StopPreview {
  token: string;
  process: ProcessDetail;
  expires_in_seconds: number;
}
export interface ExternalServiceConfiguration {
  process: ProcessDetail;
  draft: ServiceDraft;
  handoff_supported?: boolean;
}
export interface ExternalServiceHandoffPreview {
  token: string;
  process: ProcessDetail;
  service: ServiceDefinition;
  command: string;
  environment_keys: string[];
  dependencies: ServiceView[];
  expires_in_seconds: number;
}
export const defaults: Settings = {
  scan_depth: 4,
  refresh_seconds: 2,
  default_ide: 'VSCODE',
  idea_path: '',
  vscode_path: '',
  cursor_path: '',
};
export const typeNames: Record<string, string> = {
  NODE: 'Node.js',
  JAVA: 'Java',
  PYTHON: 'Python',
  RUST: 'Rust',
  GO: 'Go',
  DOTNET: '.NET',
  PHP: 'PHP',
  RUBY: 'Ruby',
  DART: 'Dart / Flutter',
  SWIFT: 'Swift / Xcode',
  CPP: 'C / C++',
  MIXED: '多语言',
};
export const ideNames: Record<string, string> = {
  IDEA: 'IntelliJ IDEA',
  VSCODE: 'VS Code',
  CURSOR: 'Cursor',
};

export type ServiceState =
  'STOPPED' | 'STARTING' | 'RUNNING' | 'READY' | 'STOPPING' | 'BLOCKED' | 'CRASHED' | 'UNKNOWN';
export type ReadinessKind = 'PROCESS' | 'PORT' | 'HTTP' | 'LOG_PHRASE';
export interface ServiceDefinition {
  id: string;
  project_id: string;
  name: string;
  runtime_type: string;
  executable: string;
  args: string[];
  shell_mode: boolean;
  working_directory: string;
  environment: Record<string, string>;
  secret_keys: string[];
  expected_ports: number[];
  dependencies: string[];
  readiness: {
    kind: ReadinessKind;
    port: number | null;
    url: string;
    phrase: string;
    timeout_seconds: number;
    interval_ms: number;
    grace_ms: number;
  };
  stop_grace_seconds: number;
  log_limit_kb: number;
  revision: string;
  created_at: number;
  updated_at: number;
}
export interface ServiceRun {
  id: string;
  service_id: string;
  project_id: string;
  revision: string;
  launch_source: 'DEVNOOK';
  workspace_run_id: string | null;
  state: ServiceState;
  pid: number | null;
  worker_pid: number | null;
  pgid: number | null;
  worker_start: string | null;
  started_at: number;
  stopped_at: number | null;
  ready_at: number | null;
  exit_code: number | null;
  error: string | null;
  log_path: string;
  stop_requested: boolean;
  stop_requested_at_ms: number | null;
  log_truncated: boolean;
}
export interface ServiceView {
  definition: ServiceDefinition;
  current_run: ServiceRun | null;
}
export interface ServiceDraft {
  definition: ServiceDefinition;
  command: string;
  source: string;
}
export interface ServiceActionPreview {
  token: string;
  action: 'start' | 'restart' | 'force_stop';
  service: ServiceDefinition;
  run: ServiceRun | null;
  command: string;
  environment_keys: string[];
  expires_in_seconds: number;
}
export interface LogPage {
  run_id: string;
  lines: string[];
  truncated: boolean;
}

export interface WorkspaceGroup {
  id: string;
  name: string;
  description: string;
  service_ids: string[];
  revision: string;
  created_at: number;
  updated_at: number;
}
export interface WorkspaceRun {
  id: string;
  workspace_id: string;
  revision: string;
  state: string;
  plan: ServiceView[];
  owned_run_ids: string[];
  reused_service_ids: string[];
  cancel_requested: boolean;
  error: string | null;
  started_at: number;
  finished_at: number | null;
}
export interface WorkspacePreview {
  token: string;
  action: string;
  workspace: WorkspaceGroup;
  plan: ServiceView[];
  previous_run_id: string | null;
  owned_run_ids: string[];
  reused_service_ids: string[];
  issued_at: number;
  expires_in_seconds: number;
}
export interface AgentGrant {
  id: string;
  name: string;
  services: Record<string, string>;
  workspaces: Record<string, string>;
  project_ids: string[];
  scan_roots: string[];
  revoked: boolean;
  created_at: number;
}
export interface GrantInput {
  scan_roots: string[];
  name: string;
  service_ids: string[];
  workspace_ids: string[];
  project_ids: string[];
}
export interface AccessAudit {
  origin: string;
  client_id: string;
  action: string;
  subject: string;
  result: string;
  created_at: number;
}
