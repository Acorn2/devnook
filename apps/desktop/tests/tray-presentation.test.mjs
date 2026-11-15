import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const modules = new Map();
async function moduleUrl(file) {
  if (modules.has(file)) return modules.get(file);
  let source = ts.transpileModule(
    await readFile(new URL(`../src/${file}.ts`, import.meta.url), 'utf8'),
    {
      compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
    },
  ).outputText;
  const dependencies = [...source.matchAll(/from ['"]\.\/([^'"]+)['"]/g)];
  for (const [statement, dependency] of dependencies)
    source = source.replace(statement, `from ${JSON.stringify(await moduleUrl(dependency))}`);
  const url = `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
  modules.set(file, url);
  return url;
}
const { initializeLanguage } = await import(await moduleUrl('i18n'));
const {
  trayGroups,
  trayEntryPorts,
  trayEntryStatus,
  reconcileTrayGroups,
  filterTrayGroups,
  trayBrowserShortcut,
} = await import(await moduleUrl('TrayPresentation'));
globalThis.localStorage = { getItem: () => null };
initializeLanguage('zh');
const project = (id, favorite = false) => ({
  id,
  name: id,
  is_favorite: favorite,
  canonical_path: `/Projects/${id}`,
});
const service = (id, state, projectId = 'p') => ({
  definition: { id, name: id, project_id: projectId, expected_ports: [9000] },
  current_run: state ? { id: `run-${id}`, state } : null,
});
const port = (overrides = {}) => ({
  pid: 10,
  port: 4321,
  project_id: 'p',
  service_id: null,
  source: 'OBSERVED',
  process_kind: 'SERVICE',
  cwd: '/Projects/p',
  ...overrides,
});

test('quick access includes active and attention states, plus inactive services only in favorite projects', () => {
  const states = [
    undefined,
    'STOPPED',
    'STARTING',
    'RUNNING',
    'READY',
    'STOPPING',
    'BLOCKED',
    'CRASHED',
    'UNKNOWN',
  ];
  const services = states.map((state, index) => service(String(index), state));
  assert.deepEqual(
    new Set(trayGroups([project('p')], services, [])[0].entries.map((e) => e.state)),
    new Set(states.slice(2)),
  );
  assert.equal(trayGroups([project('p', true)], services, [])[0].entries.length, states.length);
  assert.deepEqual(trayGroups([project('p')], [service('stopped', 'STOPPED')], []), []);
});

test('managed ports require actual Core service ownership, not an expected port or matching project', () => {
  const ports = [
    port(),
    port({ source: 'DEVNOOK', service_id: 'other' }),
    port({ source: 'DEVNOOK', service_id: 's', port: 9001 }),
    port({ source: 'DEVNOOK', service_id: 's', process_kind: 'DEVELOPMENT_TOOL', port: 9002 }),
  ];
  const entry = trayGroups([project('p')], [service('s', 'READY')], ports)[0].entries.find(
    (e) => e.key === 's',
  );
  assert.deepEqual(
    entry.bindings.map((p) => p.port),
    [9001],
  );
  assert.deepEqual(entry.target, { kind: 'MANAGED', service_id: 's' });
  assert.deepEqual(entry.destination, { kind: 'managed', id: 's', action: null });
});

test('external rows deduplicate dual stack by PID and exclude tools, unknown purpose and unknown project', () => {
  const ports = [
    port(),
    port({ family: 'IPv6', address: '::1' }),
    port({ pid: 11, process_kind: 'DEVELOPMENT_TOOL' }),
    port({ pid: 12, process_kind: 'UNKNOWN' }),
    port({ pid: 13, project_id: null }),
    port({ pid: 14, project_id: 'missing' }),
  ];
  const entries = trayGroups([project('p')], [], ports)[0].entries;
  assert.equal(entries.length, 1);
  assert.equal(entries[0].bindings.length, 2);
  assert.equal(entries[0].state, undefined, 'external listening is not managed READY');
  assert.equal(entries[0].external, true);
  assert.deepEqual(entries[0].destination, { kind: 'external', pid: 10 });
});

test('attention sorts first and grouping does not mutate Core snapshots', () => {
  const projects = [project('a', true), project('p')];
  const services = [
    service('ready', 'READY', 'a'),
    service('stopped', 'STOPPED', 'a'),
    service('crash', 'CRASHED'),
  ];
  const original = structuredClone({ projects, services });
  const groups = trayGroups(projects, services, []);
  assert.deepEqual(
    groups.map((g) => g.id),
    ['p', 'a'],
  );
  assert.deepEqual(
    groups[1].entries.map((e) => e.key),
    ['ready', 'stopped'],
  );
  assert.deepEqual({ projects, services }, original);
});

test('missing project and localized fallback preserve names and keep running services visible', () => {
  initializeLanguage('en');
  const groups = trayGroups([], [service('中文服务', 'READY', 'removed')], []);
  assert.equal(groups[0].name, 'Removed project');
  assert.equal(groups[0].entries[0].name, '中文服务');
  initializeLanguage('zh');
});

test('compact rows separate external names from actual ports without inventing a service role', () => {
  const entries = trayGroups(
    [project('p')],
    [],
    [port(), port({ port: 4322 }), port({ family: 'IPv6' })],
  )[0].entries;
  assert.equal(entries[0].name, '外部服务');
  assert.equal(trayEntryPorts(entries[0]), ':4321 · :4322');
  assert.equal(trayEntryStatus(entries[0]), '');
  const component = trayGroups([project('p')], [], [port({ cwd: '/Projects/p/apps/preview' })])[0]
    .entries[0];
  assert.equal(component.name, './apps/preview');
  assert.equal(trayEntryPorts(component), ':4321');
  assert.equal(trayEntryStatus(component), '外部');
});

test('compact labels keep unready and unverified runs distinct from readiness', () => {
  const entries = trayGroups(
    [project('p', true)],
    [service('running', 'RUNNING'), service('unknown', 'UNKNOWN'), service('never', undefined)],
    [],
  )[0].entries;
  assert.equal(trayEntryStatus(entries.find((entry) => entry.key === 'running')), '未就绪');
  assert.equal(trayEntryStatus(entries.find((entry) => entry.key === 'unknown')), '待核验');
  assert.equal(trayEntryStatus(entries.find((entry) => entry.key === 'never')), '未启动');
  initializeLanguage('en');
  assert.equal(trayEntryStatus(entries.find((entry) => entry.key === 'unknown')), 'Unverified');
  initializeLanguage('zh');
});

test('an open session keeps group and row positions, disables missing entries, and resets next opening', () => {
  const first = trayGroups(
    [project('a', true), project('p')],
    [service('one', 'READY', 'a'), service('two', 'READY')],
    [],
  );
  const session = reconcileTrayGroups(null, first);
  assert.ok(session.every((g) => g.compact));
  const changed = trayGroups(
    [project('a', true), project('p')],
    [service('one', 'STOPPED', 'a'), service('new', 'CRASHED'), service('two', 'CRASHED')],
    [],
  );
  const merged = reconcileTrayGroups(session, changed);
  assert.deepEqual(
    merged.map((g) => g.id),
    ['a', 'p'],
  );
  assert.deepEqual(
    merged[1].entries.map((e) => e.key),
    ['two', 'new'],
  );
  const gone = reconcileTrayGroups(merged, []);
  assert.ok(gone.flatMap((g) => g.entries).every((e) => e.unavailable && !e.bindings.length));
  assert.equal(trayEntryStatus(gone[0].entries[0]), '已不可用');
  assert.deepEqual(
    reconcileTrayGroups(null, changed).map((g) => g.id),
    ['p', 'a'],
  );
});

test('search combines project, service and actual port without changing single/multi grouping', () => {
  const groups = reconcileTrayGroups(
    null,
    trayGroups(
      [project('p', true)],
      [service('backend', 'READY'), service('worker', 'READY')],
      [port({ source: 'DEVNOOK', service_id: 'backend' })],
    ),
  );
  const filtered = filterTrayGroups(groups, ' P 4321 ');
  assert.equal(filtered[0].entries[0].name, 'backend');
  assert.equal(filtered[0].compact, false);
  assert.deepEqual(
    filterTrayGroups(groups, '9000'),
    [],
    'expected ports are not searchable actual listeners',
  );
  assert.equal(filterTrayGroups(groups, 'WORKER')[0].entries.length, 1);
  assert.equal(groups[0].entries.length, 2);
});

test('tray browser shortcut requires confirmed page, matching target and current actual PID/port; ambiguity goes to details', () => {
  const entry = trayGroups([project('p')], [], [port()])[0].entries[0];
  const shortcut = (overrides = {}, target = entry.target) => ({
    target,
    address: {
      url: 'http://127.0.0.1:4321/',
      pid: 10,
      fingerprint: 'identity',
      web_ui: true,
      ...overrides,
    },
  });
  assert.equal(trayBrowserShortcut(entry, [shortcut()]).web_ui, true);
  for (const value of [undefined, null, false])
    assert.equal(trayBrowserShortcut(entry, [shortcut({ web_ui: value })]), undefined);
  assert.equal(trayBrowserShortcut(entry, [shortcut({ pid: 11 })]), undefined);
  assert.equal(
    trayBrowserShortcut(entry, [shortcut({ url: 'http://127.0.0.1:5001/' })]),
    undefined,
  );
  assert.equal(
    trayBrowserShortcut(entry, [shortcut({}, { kind: 'MANAGED', service_id: 'unrelated' })]),
    undefined,
  );
  assert.equal(trayBrowserShortcut({ ...entry, unavailable: true }, [shortcut()]), undefined);
  const multi = { ...entry, bindings: [...entry.bindings, port({ port: 4322 })] };
  assert.equal(
    trayBrowserShortcut(multi, [shortcut(), shortcut({ url: 'http://127.0.0.1:4322/' })]),
    undefined,
  );
  assert.ok(trayBrowserShortcut(entry, [shortcut(), shortcut({ url: 'http://[::1]:4321/' })]));
});

test('an existing session translates fallback labels immediately while preserving project and saved names', () => {
  initializeLanguage('zh');
  const groups = reconcileTrayGroups(
    null,
    trayGroups(
      [project('外部服务')],
      [],
      [port({ project_id: '外部服务', cwd: '/Projects/外部服务' })],
    ),
  );
  initializeLanguage('en');
  assert.equal(filterTrayGroups(groups, '')[0].name, '外部服务');
  assert.equal(filterTrayGroups(groups, '')[0].entries[0].name, 'External services');
  assert.equal(filterTrayGroups(groups, 'external').length, 1);
  initializeLanguage('zh');
  assert.equal(filterTrayGroups(groups, '')[0].entries[0].name, '外部服务');
});
