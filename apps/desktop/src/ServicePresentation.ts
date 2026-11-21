import { t } from './i18n';
import type {
  PortBinding,
  Project,
  ServiceDefinition,
  ServiceDraft,
  ServiceState,
  ServiceView,
} from './types';

type ProjectIdentity = Pick<Project, 'name' | 'canonical_path'>;
export interface ServiceIdentity {
  title: string;
  qualifier: string;
}

export const serviceIsActive = (state?: ServiceState) =>
  !!state && ['STARTING', 'RUNNING', 'READY', 'STOPPING', 'UNKNOWN'].includes(state);

const serviceLabels: Record<ServiceState, string> = {
  STOPPED: '已停止',
  STARTING: '正在启动',
  RUNNING: '运行中 · 未就绪',
  READY: '已就绪',
  STOPPING: '正在停止',
  BLOCKED: '启动被阻断',
  CRASHED: '运行失败',
  UNKNOWN: '身份待核验',
};
export const serviceStateLabel = (state?: ServiceState) =>
  t(state ? serviceLabels[state] : '未启动');

export function pendingServiceDrafts(
  drafts: ServiceDraft[],
  services: ServiceView[],
  query: string,
) {
  const search = query.toLowerCase();
  return drafts.filter(
    ({ definition, command }) =>
      !services.some(
        ({ definition: saved }) =>
          saved.project_id === definition.project_id &&
          saved.working_directory === definition.working_directory,
      ) &&
      [definition.name, command, definition.working_directory]
        .join(' ')
        .toLowerCase()
        .includes(search),
  );
}

export function groupServiceDrafts(pending: ServiceDraft[], drafts: ServiceDraft[], root?: string) {
  const hasChildDraft =
    !!root && drafts.some((draft) => draft.definition.working_directory !== root);
  const primaryDrafts: ServiceDraft[] = [];
  const otherDrafts: ServiceDraft[] = [];
  for (const draft of pending) {
    const isOther =
      hasChildDraft && draft.definition.working_directory === root && !draft.independent_entrypoint;
    (isOther ? otherDrafts : primaryDrafts).push(draft);
  }
  return { primaryDrafts, otherDrafts };
}

// These are display choices, never process ownership or permission to stop one.
// A user selects an instance; Core independently validates and binds the preview.
export function externalServiceChoices(
  ports: PortBinding[],
  projectId: string,
  available: boolean,
) {
  if (!available) return [];
  const groups = new Map<number, PortBinding[]>();
  for (const binding of ports) {
    if (
      binding.source !== 'OBSERVED' ||
      binding.process_kind !== 'SERVICE' ||
      binding.project_id !== projectId
    )
      continue;
    const group = groups.get(binding.pid) || [];
    group.push(binding);
    groups.set(binding.pid, group);
  }
  return [...groups.values()].sort((a, b) => a[0].pid - b[0].pid);
}

export function externalPortConflict(service: ServiceDefinition, choices: PortBinding[][]) {
  return choices.some((bindings) => bindings.some((b) => service.expected_ports.includes(b.port)));
}

export function serviceCliCommands(serviceId: string) {
  const id = "'" + serviceId.replaceAll("'", "'\\''") + "'";
  return ['status', 'logs', 'start', 'restart', 'stop']
    .map((action) => `devnook service ${action} ${id}${action === 'logs' ? ' --limit 100' : ''}`)
    .join('\n');
}

export function serviceAgentInstructions(serviceId: string) {
  return [
    t('此服务已登记在 DevNook，后续通过已有配置管理。'),
    `service_id: ${JSON.stringify(serviceId)}`,
    t('优先使用已连接的 DevNook MCP；未接入时使用 CLI，先确认可执行文件路径。'),
    t('先查询状态和日志；需要启停或重启时，按用户指令及现有预览、确认和授权规则执行。'),
    t('不要重复执行旧终端启动命令、直接终止 PID，或因旧终端退出而自动拉起另一份服务。'),
    t('任务结束不默认停止服务，尤其不要停止只是复用的既有运行。'),
    t('配置或启动方式改变时，先提示核对 DevNook 配置。'),
    serviceCliCommands(serviceId),
    t('CLI 未加入 PATH 时，替换为已确认的绝对路径。非交互写操作仅在得到具体操作授权后使用 --yes。'),
  ].join('\n');
}

export function relativeServiceDirectory(directory: string, project?: ProjectIdentity) {
  const root = project?.canonical_path;
  if (directory === root) return '.';
  return root && directory.startsWith(root + '/') ? '.' + directory.slice(root.length) : directory;
}

export function managedServiceIdentity(
  definition: Pick<ServiceDefinition, 'name' | 'working_directory'>,
  project: ProjectIdentity | undefined,
  projectScoped: boolean,
): ServiceIdentity {
  const directory = relativeServiceDirectory(definition.working_directory, project);
  return {
    title: projectScoped ? definition.name : project?.name || t('已移除的项目'),
    qualifier: [definition.name, directory !== '.' && directory].filter(Boolean).join(' · '),
  };
}

export function externalServiceIdentity(
  bindings: PortBinding[],
  project: ProjectIdentity | undefined,
  projectScoped: boolean,
  portsAvailable: boolean,
): ServiceIdentity {
  const process = bindings[0];
  const directory = relativeServiceDirectory(process.cwd || '', project);
  // Only a path inside the associated project can name a component. Runtime
  // names and ports do not provide evidence of a frontend/backend role.
  const ports = [...new Set(bindings.map((binding) => binding.port))]
    .sort((a, b) => a - b)
    .map((port) => `:${port}`)
    .join(' · ');
  const portLabel = portsAvailable ? ports : t('端口状态未知');
  const qualifier = directory.startsWith('./')
    ? `${directory} · ${portLabel}`
    : t('外部服务 · {{0}}', [portLabel]);
  return {
    title: projectScoped ? qualifier : project?.name || t('未关联项目'),
    qualifier,
  };
}
