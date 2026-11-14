import type { PortBinding, Project } from './types';

// Presentation groups describe the reported process name, not a project's stack
// or its ownership. They must never be used to decide whether a PID can stop.
export const runtimeOptions = [
  { value: 'ALL', label: '全部运行时' },
  { value: 'NODE', label: 'Node.js / JavaScript' },
  { value: 'PYTHON', label: 'Python' },
  { value: 'JAVA', label: 'Java / JVM' },
  { value: 'BUN', label: 'Bun' },
  { value: 'DENO', label: 'Deno' },
  { value: 'DOTNET', label: '.NET' },
  { value: 'PHP', label: 'PHP' },
  { value: 'RUBY', label: 'Ruby' },
  { value: 'OTHER', label: '其他 / 无法识别' },
];

export function portRuntime(name: string): string {
  const process = name.split('/').pop()?.toLowerCase() || '';
  if (/^(node|nodejs)(\d+(\.\d+)*)?$/.test(process)) return 'NODE';
  if (/^python(w)?(\d+(\.\d+)*)?$/.test(process)) return 'PYTHON';
  if (process === 'java') return 'JAVA';
  if (process === 'bun') return 'BUN';
  if (process === 'deno') return 'DENO';
  if (process === 'dotnet') return 'DOTNET';
  if (/^php(-fpm)?(\d+(\.\d+)*)?$/.test(process)) return 'PHP';
  if (/^ruby(\d+(\.\d+)*)?$/.test(process)) return 'RUBY';
  return 'OTHER';
}

export interface PortFilters {
  query: string;
  project: string;
  family: string;
  runtime: string;
  kind: string;
  source: string;
}

export function filterPortBindings(
  ports: PortBinding[],
  projects: Project[],
  filters: PortFilters,
) {
  const names = new Map(projects.map((p) => [p.id, p.name]));
  return ports.filter((p) => {
    if (
      filters.project === 'EXTERNAL'
        ? p.project_id !== null
        : filters.project !== 'ALL' && p.project_id !== filters.project
    )
      return false;
    if (filters.family !== 'ALL' && p.family !== filters.family) return false;
    if (filters.runtime !== 'ALL' && portRuntime(p.name) !== filters.runtime) return false;
    if (filters.kind !== 'ALL' && (p.process_kind || 'UNKNOWN') !== filters.kind) return false;
    const source = p.source === 'DEVNOOK' ? 'DEVNOOK' : 'OBSERVED';
    if (filters.source !== 'ALL' && source !== filters.source) return false;
    return [
      p.port,
      p.pid,
      p.name,
      p.address,
      p.command,
      p.cwd,
      names.get(p.project_id || '') || '未关联项目 Unassigned project',
    ]
      .join(' ')
      .toLowerCase()
      .includes(filters.query.toLowerCase());
  });
}

export function portRows(bindings: PortBinding[]) {
  return [...new Map(bindings.map((p) => [`${p.pid}:${p.port}`, p])).values()];
}

export function conflictingBindings(ports: PortBinding[], expected: number[], serviceId: string) {
  return ports.filter(
    (p) => expected.includes(p.port) && !(p.source === 'DEVNOOK' && p.service_id === serviceId),
  );
}
