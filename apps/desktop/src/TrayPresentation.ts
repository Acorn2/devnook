import { dateLocale, t } from './i18n';
import { relativeServiceDirectory, serviceIsActive } from './ServicePresentation';
import { preferredHttpAddresses } from './BrowserPresentation';
import type {
  BrowserShortcut,
  BrowserTarget,
  PortBinding,
  Project,
  ServiceState,
  ServiceView,
} from './types';

export type MainDestination =
  | { kind: 'services' }
  | { kind: 'managed'; id: string; action: 'start' | 'restart' | null }
  | { kind: 'external'; pid: number };
export interface MainNavigation {
  sequence: number;
  destination: MainDestination;
}
export interface TrayGroup {
  id: string;
  name: string;
  favorite: boolean;
  entries: TrayEntry[];
  compact?: boolean;
  nameKey?: '已移除的项目';
}

// Freeze positions and grouping for one opening. Disappearing entries lose all
// actions immediately but keep a slot until the next opening to avoid misclicks.
export function reconcileTrayGroups(
  previous: TrayGroup[] | null,
  current: TrayGroup[],
): TrayGroup[] {
  if (!previous) return current.map((g) => ({ ...g, compact: g.entries.length === 1 }));
  const remaining = new Map(current.map((g) => [g.id, g]));
  const result = previous.map((group) => {
    const fresh = remaining.get(group.id);
    remaining.delete(group.id);
    const entries = new Map(fresh?.entries.map((e) => [e.key, e]) || []);
    const stable = group.entries.map((entry) => {
      const value = entries.get(entry.key);
      entries.delete(entry.key);
      return value || { ...entry, unavailable: true, bindings: [], state: undefined };
    });
    return {
      ...group,
      name: fresh?.name || group.name,
      nameKey: fresh ? fresh.nameKey : group.nameKey,
      entries: [...stable, ...entries.values()],
    };
  });
  return [
    ...result,
    ...[...remaining.values()].map((g) => ({ ...g, compact: g.entries.length === 1 })),
  ];
}

export function filterTrayGroups(groups: TrayGroup[], query: string): TrayGroup[] {
  groups = groups.map((group) => ({
    ...group,
    name: group.nameKey ? t(group.nameKey) : group.name,
    entries: group.entries.map((entry) => ({
      ...entry,
      name: entry.nameKey ? t(entry.nameKey) : entry.name,
    })),
  }));
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return groups;
  return groups
    .map((g) => ({
      ...g,
      entries: g.entries.filter((e) => {
        const text = `${g.name} ${e.name} ${trayEntryPorts(e)}`.toLocaleLowerCase();
        return words.every((word) => text.includes(word));
      }),
    }))
    .filter((g) => g.entries.length);
}

export function trayBrowserShortcut(entry: TrayEntry, shortcuts: BrowserShortcut[]) {
  if (
    entry.unavailable ||
    (!entry.external && !['STARTING', 'RUNNING', 'READY'].includes(entry.state || ''))
  )
    return undefined;
  const choices = preferredHttpAddresses(
    shortcuts
      .filter(({ target, address }) => {
        const sameTarget =
          target.kind === entry.target.kind &&
          (target.kind === 'MANAGED'
            ? entry.target.kind === 'MANAGED' && target.service_id === entry.target.service_id
            : entry.target.kind === 'EXTERNAL' && target.pid === entry.target.pid);
        if (!sameTarget || address.web_ui !== true) return false;
        try {
          const port = Number(new URL(address.url).port || '80');
          return entry.bindings.some((b) => b.pid === address.pid && b.port === port);
        } catch {
          return false;
        }
      })
      .map(({ address }) => address),
  );
  return choices.length === 1 ? choices[0] : undefined;
}
export interface TrayEntry {
  key: string;
  name: string;
  state?: ServiceState;
  external: boolean;
  bindings: PortBinding[];
  target: BrowserTarget;
  destination: MainDestination;
  unavailable?: boolean;
  nameKey?: '外部服务';
}
export const needsAttention = (state?: ServiceState) =>
  !!state && ['BLOCKED', 'CRASHED', 'UNKNOWN'].includes(state);

export function trayEntryPorts(entry: TrayEntry) {
  return [...new Set(entry.bindings.map((binding) => binding.port))]
    .sort((a, b) => a - b)
    .map((port) => `:${port}`)
    .join(' · ');
}

export function trayEntryStatus(entry: TrayEntry) {
  if (entry.unavailable) return t('已不可用');
  // Root listeners already say "External service"; component paths need the label.
  if (entry.external) return entry.name.startsWith('./') ? t('外部') : '';
  if (!entry.state) return t('未启动');
  const labels: Record<ServiceState, string> = {
    STOPPED: '已停止',
    STARTING: '启动中',
    RUNNING: '未就绪',
    READY: '已就绪',
    STOPPING: '停止中',
    BLOCKED: '已阻断',
    CRASHED: '失败',
    UNKNOWN: '待核验',
  };
  return t(labels[entry.state]);
}

export function trayGroups(projects: Project[], services: ServiceView[], ports: PortBinding[]) {
  const groups = new Map<string, TrayGroup>();
  function group(id: string) {
    const project = projects.find((p) => p.id === id);
    if (!groups.has(id))
      groups.set(id, {
        id,
        name: project?.name || t('已移除的项目'),
        nameKey: project ? undefined : '已移除的项目',
        favorite: !!project?.is_favorite,
        entries: [],
      });
    return groups.get(id)!;
  }
  for (const { definition, current_run } of services) {
    const state = current_run?.state;
    const project = projects.find((p) => p.id === definition.project_id);
    if (!serviceIsActive(state) && !needsAttention(state) && !project?.is_favorite) continue;
    group(definition.project_id).entries.push({
      key: definition.id,
      name: definition.name,
      state,
      external: false,
      bindings: ports.filter(
        (p) =>
          p.process_kind === 'SERVICE' && p.source === 'DEVNOOK' && p.service_id === definition.id,
      ),
      target: { kind: 'MANAGED', service_id: definition.id },
      destination: { kind: 'managed', id: definition.id, action: null },
    });
  }
  const external = new Map<number, PortBinding[]>();
  for (const port of ports) {
    if (port.process_kind !== 'SERVICE' || port.source !== 'OBSERVED' || !port.project_id) continue;
    const bindings = external.get(port.pid) || [];
    bindings.push(port);
    external.set(port.pid, bindings);
  }
  for (const [pid, bindings] of external) {
    const project = projects.find((p) => p.id === bindings[0].project_id);
    if (!project) continue;
    const directory = relativeServiceDirectory(bindings[0].cwd || '', project);
    group(project.id).entries.push({
      key: `external:${pid}`,
      name: directory.startsWith('./') ? directory : t('外部服务'),
      nameKey: directory.startsWith('./') ? undefined : '外部服务',
      external: true,
      bindings,
      target: { kind: 'EXTERNAL', pid },
      destination: { kind: 'external', pid },
    });
  }
  const rank = (entry: TrayEntry) =>
    needsAttention(entry.state) ? 0 : entry.external || serviceIsActive(entry.state) ? 1 : 2;
  for (const group of groups.values())
    group.entries.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name, dateLocale()));
  return [...groups.values()].sort(
    (a, b) =>
      Number(b.entries.some((e) => needsAttention(e.state))) -
        Number(a.entries.some((e) => needsAttention(e.state))) ||
      Number(b.favorite) - Number(a.favorite) ||
      a.name.localeCompare(b.name, dateLocale()),
  );
}
