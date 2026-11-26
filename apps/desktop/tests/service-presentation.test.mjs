import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const compile = (source) =>
  ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
const dataUrl = (source) => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const load = async (file) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8');
const catalogUrl = dataUrl(compile(await load('locales/en.ts')));
const i18nUrl = dataUrl(
  compile(await load('i18n.ts')).replace(
    /from ['"]\.\/locales\/en['"]/,
    `from ${JSON.stringify(catalogUrl)}`,
  ),
);
const { initializeLanguage } = await import(i18nUrl);
const {
  managedServiceIdentity,
  externalServiceIdentity,
  relativeServiceDirectory,
  externalServiceChoices,
  externalPortConflict,
  serviceCliCommands,
  serviceAgentInstructions,
  serviceIsActive,
  formatMemoryBytes,
  formatCpuTime,
  formatLogTimestamp,
  classifyLogStream,
  filterLogLines,
  highlightTextParts,
  isSensitiveEnvKey,
  splitSensitiveEnvLines,
  groupServicesByProject,
  duplicateServiceDefinition,
  groupServiceDrafts,
  pendingServiceDrafts,
} = await import(
  dataUrl(
    compile(await load('ServicePresentation.ts')).replace(
      /from ['"]\.\/i18n['"]/,
      `from ${JSON.stringify(i18nUrl)}`,
    ),
  )
);
globalThis.localStorage = { getItem: () => null };
initializeLanguage('zh-CN');

const project = { name: '开发工作台', canonical_path: '/Projects/workbench' };
const definition = { name: 'backend', working_directory: '/Projects/workbench/apps/api' };
const binding = (changes = {}) => ({
  pid: 41,
  name: 'node',
  project_id: 'workbench',
  cwd: project.canonical_path,
  port: 4310,
  address: '127.0.0.1',
  family: 'IPv4',
  source: 'OBSERVED',
  service_id: null,
  process_kind: 'SERVICE',
  command: 'node server.js',
  first_seen: 0,
  ...changes,
});

test('a saved Vite service hides only its own draft, not the FastAPI draft at the project root', () => {
  const backend = {
    definition: {
      name: 'FastAPI',
      project_id: 'workbench',
      working_directory: project.canonical_path,
    },
    command: 'python3 -m server',
    independent_entrypoint: true,
  };
  const frontend = {
    definition: {
      name: 'web',
      project_id: 'workbench',
      working_directory: project.canonical_path + '/web',
    },
    command: 'npm run dev',
  };
  const drafts = [frontend, backend];
  const saved = [
    { definition: frontend.definition },
    { definition: { ...backend.definition, project_id: 'other' } },
  ];
  assert.deepEqual(pendingServiceDrafts(drafts, saved, ''), [backend]);
  assert.deepEqual(groupServiceDrafts([backend], drafts, project.canonical_path), {
    primaryDrafts: [backend],
    otherDrafts: [],
  });
  assert.deepEqual(pendingServiceDrafts(drafts, saved, 'PYTHON3'), [backend]);
  assert.deepEqual(pendingServiceDrafts(drafts, saved, 'vite'), []);
  assert.deepEqual(
    pendingServiceDrafts(drafts, [...saved, { definition: backend.definition }], ''),
    [],
  );
});

test('an unverified root launch stays separate from child suggestions, including on older Core versions', () => {
  const root = project.canonical_path;
  const child = { definition: { working_directory: `${root}/web` }, command: 'npm run dev' };
  for (const command of ['', 'npm run dev:all']) {
    const rootDraft = { definition: { working_directory: root }, command };
    const drafts = [child, rootDraft];
    assert.deepEqual(groupServiceDrafts(drafts, drafts, root), {
      primaryDrafts: [child],
      otherDrafts: [rootDraft],
    });
    assert.deepEqual(groupServiceDrafts([rootDraft], drafts, root), {
      primaryDrafts: [],
      otherDrafts: [rootDraft],
    });
    assert.deepEqual(groupServiceDrafts([rootDraft], [rootDraft], root), {
      primaryDrafts: [rootDraft],
      otherDrafts: [],
    });
  }
});

test('global managed records lead with the project; project records lead with the saved service name', () => {
  assert.deepEqual(managedServiceIdentity(definition, project, false), {
    title: '开发工作台',
    qualifier: 'backend · ./apps/api',
  });
  assert.deepEqual(managedServiceIdentity(definition, project, true), {
    title: 'backend',
    qualifier: 'backend · ./apps/api',
  });
  const worker = { ...definition, working_directory: '/Projects/workbench/apps/worker' };
  assert.notEqual(
    managedServiceIdentity(definition, project, false).qualifier,
    managedServiceIdentity(worker, project, false).qualifier,
  );
  assert.equal(
    managedServiceIdentity(
      { ...definition, working_directory: project.canonical_path },
      project,
      false,
    ).qualifier,
    'backend',
  );
});

test('external records use component paths and actual ports without inventing a runtime role', () => {
  const bindings = [binding({ cwd: '/Projects/workbench/site' })];
  const before = structuredClone(bindings);
  assert.deepEqual(externalServiceIdentity(bindings, project, false, true), {
    title: '开发工作台',
    qualifier: './site · :4310',
  });
  assert.equal(externalServiceIdentity(bindings, project, true, true).title, './site · :4310');
  assert.deepEqual(bindings, before);
});

test('root services with no saved name remain distinguishable by listening port and dual stack stays one identity', () => {
  const first = [binding(), binding({ family: 'IPv6', address: '::1' }), binding({ port: 4311 })];
  assert.equal(
    externalServiceIdentity(first, project, true, true).title,
    '外部服务 · :4310 · :4311',
  );
  assert.notEqual(
    externalServiceIdentity(first, project, true, true).title,
    externalServiceIdentity([binding({ pid: 42, port: 4320 })], project, true, true).title,
  );
});

test('unavailable snapshots never present stale ports as current in a record name', () => {
  assert.equal(
    externalServiceIdentity([binding()], project, true, false).title,
    '外部服务 · 端口状态未知',
  );
});

test('missing projects and paths outside the associated root do not become guessed project or component names', () => {
  assert.equal(
    relativeServiceDirectory('/Projects/workbench-copy/site', project),
    '/Projects/workbench-copy/site',
  );
  for (const cwd of [null, '/Projects/workbench-copy/site', '/Other/site']) {
    assert.equal(
      externalServiceIdentity([binding({ cwd })], project, true, true).title,
      '外部服务 · :4310',
    );
  }
  assert.equal(externalServiceIdentity([binding()], undefined, false, true).title, '未关联项目');
  assert.equal(managedServiceIdentity(definition, undefined, false).title, '已移除的项目');
});

test('fallback labels switch language while project names, saved names and directory values stay literal', () => {
  initializeLanguage('en-US');
  assert.equal(
    externalServiceIdentity([binding()], project, true, true).title,
    'External service · :4310',
  );
  assert.equal(managedServiceIdentity(definition, project, false).title, '开发工作台');
  assert.equal(managedServiceIdentity(definition, project, true).title, 'backend');
  assert.equal(
    externalServiceIdentity([binding({ cwd: '/Projects/workbench/中文目录' })], project, true, true)
      .title,
    './中文目录 · :4310',
  );
  initializeLanguage('zh-CN');
});

test('handoff display choices use current observed service facts, with one choice per PID', () => {
  const ports = [
    binding({ project_id: 'a', pid: 42 }),
    binding({ project_id: 'a', pid: 42, family: 'IPv6', address: '::1' }),
    binding({ project_id: 'a', pid: 43 }),
    binding({ project_id: 'other', pid: 44 }),
    binding({ project_id: 'a', source: 'DEVNOOK', pid: 45 }),
    binding({ project_id: 'a', process_kind: 'DEVELOPMENT_TOOL', pid: 46 }),
    binding({ project_id: 'a', process_kind: 'UNKNOWN', pid: 47 }),
    binding({ project_id: null, pid: 48 }),
  ];
  assert.deepEqual(
    externalServiceChoices(ports, 'a', true).map((g) => g.map((b) => b.pid)),
    [[42, 42], [43]],
  );
  assert.deepEqual(externalServiceChoices(ports, 'a', false), []);
  assert.deepEqual(externalServiceChoices(ports, 'missing', true), []);
  assert.equal(
    externalPortConflict({ expected_ports: [4310] }, externalServiceChoices(ports, 'a', true)),
    true,
  );
  assert.equal(
    externalPortConflict({ expected_ports: [9000] }, externalServiceChoices(ports, 'a', true)),
    false,
  );
  assert.equal(
    externalPortConflict({ expected_ports: [] }, externalServiceChoices(ports, 'a', true)),
    false,
  );
});

test('active managed runs never become selectable takeover targets, including unknown identity', () => {
  for (const state of ['STARTING', 'RUNNING', 'READY', 'STOPPING', 'UNKNOWN'])
    assert.equal(serviceIsActive(state), true);
  for (const state of [undefined, 'STOPPED', 'BLOCKED', 'CRASHED'])
    assert.equal(serviceIsActive(state), false);
});

test('AI handoff uses the saved service ID and current CLI, without shell launch or implicit approval', () => {
  initializeLanguage('zh');
  const id = "id';$(do-not-execute)";
  const commands = serviceCliCommands(id);
  assert.equal(commands.split('\n').length, 5);
  assert.ok(commands.includes("service status 'id'\\'';$(do-not-execute)'"));
  assert.ok(commands.includes(' --limit 100'));
  assert.ok(!commands.includes('--yes'));
  assert.ok(!commands.includes('kill '));
  const instructions = serviceAgentInstructions('saved-id');
  assert.ok(instructions.includes('service_id: "saved-id"'));
  assert.ok(instructions.includes('不要重复执行旧终端启动命令'));
  assert.ok(instructions.includes('既有运行'));
  assert.ok(!instructions.includes('DEVNOOK_MCP_TOKEN='));
  initializeLanguage('en');
  assert.ok(serviceAgentInstructions('saved-id').includes('connected DevNook MCP'));
  assert.equal(
    serviceCliCommands('saved-id'),
    "devnook service status 'saved-id'\ndevnook service logs 'saved-id' --limit 100\ndevnook service start 'saved-id'\ndevnook service restart 'saved-id'\ndevnook service stop 'saved-id'",
  );
  initializeLanguage('zh');
});

test('memory and CPU time formatting handles various byte and time ranges gracefully', () => {
  assert.equal(formatMemoryBytes(undefined), '');
  assert.equal(formatMemoryBytes(null), '');
  assert.equal(formatMemoryBytes(0), '');
  assert.equal(formatMemoryBytes(512 * 1024), '512.0 KB');
  assert.equal(formatMemoryBytes(128 * 1024 * 1024), '128.0 MB');
  assert.equal(formatMemoryBytes(2.5 * 1024 * 1024 * 1024), '2.50 GB');

  assert.equal(formatCpuTime(undefined), '');
  assert.equal(formatCpuTime(null), '');
  assert.equal(formatCpuTime(0), '');
  assert.equal(formatCpuTime(1500), '1.5s');
  assert.equal(formatCpuTime(65000), '1m 5s');
  assert.equal(formatCpuTime(125000), '2m 5s');
});

test('formatLogTimestamp formats unix seconds to readable local HH:mm:ss without touching non-timestamp lines', () => {
  const line = '[1795187637] [system] 服务运行开始';
  const formatted = formatLogTimestamp(line);
  assert.match(formatted, /^\[\d{2}:\d{2}:\d{2}\] \[system\] 服务运行开始$/);

  const plainLine = 'Some raw line without timestamp';
  assert.equal(formatLogTimestamp(plainLine), plainLine);
});

test('log stream classification, filtering, and keyword highlighting work safely without regex injection', () => {
  const lines = [
    '[1795187637] [system] 服务运行开始',
    '[1795187638] [stdout] listening on http://127.0.0.1:3000 [stderr] literal text',
    '[1795187639] [stderr] Error: EADDRINUSE (port 3000)',
  ];
  assert.equal(classifyLogStream(lines[0]), 'system');
  assert.equal(classifyLogStream(lines[1]), 'stdout');
  assert.equal(classifyLogStream(lines[2]), 'stderr');
  assert.deepEqual(filterLogLines(lines, 'ALL'), lines);
  assert.deepEqual(filterLogLines(lines, 'STDERR'), [lines[2]]);
  assert.deepEqual(filterLogLines(lines, 'SYSTEM'), [lines[0]]);
  assert.deepEqual(highlightTextParts('Error: EADDRINUSE (port 3000)', '(port 3000)'), [
    { text: 'Error: EADDRINUSE ', match: false },
    { text: '(port 3000)', match: true },
  ]);
  assert.deepEqual(highlightTextParts('Hello World hello', 'HELLO'), [
    { text: 'Hello', match: true },
    { text: ' World ', match: false },
    { text: 'hello', match: true },
  ]);
});

test('splitSensitiveEnvLines separates sensitive keys and credential URLs while keeping plain and malformed lines', () => {
  assert.equal(isSensitiveEnvKey('OPENAI_API_KEY'), true);
  assert.equal(isSensitiveEnvKey('DB_PASSWORD'), true);
  assert.equal(isSensitiveEnvKey('SIGNING_KEY'), true);
  assert.equal(isSensitiveEnvKey('NODE_ENV'), false);
  const split = splitSensitiveEnvLines(
    [
      'NODE_ENV=development',
      'API_TOKEN=secret-123',
      'DATABASE_URL=postgres://user:pass@localhost:5432/app',
      'PORT=8080',
      'MALFORMED_LINE',
    ].join('\n'),
  );
  assert.deepEqual(split.plainLines, ['NODE_ENV=development', 'PORT=8080', 'MALFORMED_LINE']);
  assert.deepEqual(split.sensitiveEntries, [
    { key: 'API_TOKEN', value: 'secret-123' },
    { key: 'DATABASE_URL', value: 'postgres://user:pass@localhost:5432/app' },
  ]);
});

test('duplicateServiceDefinition clears identity and revision while cloning fields and keeping secret key names only', () => {
  const original = {
    id: 'svc-1',
    project_id: 'proj-1',
    name: 'backend',
    runtime_type: 'NODE',
    executable: 'pnpm',
    args: ['dev'],
    shell_mode: false,
    working_directory: '/Projects/workbench/api',
    environment: { NODE_ENV: 'development' },
    secret_keys: ['API_TOKEN'],
    expected_ports: [8080],
    dependencies: ['db-1'],
    readiness: {
      kind: 'PORT',
      port: 8080,
      url: '',
      phrase: '',
      timeout_seconds: 30,
      interval_ms: 500,
      grace_ms: 500,
    },
    stop_grace_seconds: 5,
    log_limit_kb: 1024,
    revision: 'rev-1',
    created_at: 100,
    updated_at: 200,
  };
  const copy = duplicateServiceDefinition(original, ' 副本');
  assert.equal(copy.id, '');
  assert.equal(copy.revision, '');
  assert.equal(copy.name, 'backend 副本');
  assert.equal(copy.created_at, 0);
  assert.equal(copy.updated_at, 0);
  assert.deepEqual(copy.secret_keys, ['API_TOKEN']);
  copy.args.push('--port');
  copy.environment.PORT = '9000';
  copy.expected_ports.push(9000);
  assert.deepEqual(original.args, ['dev']);
  assert.deepEqual(original.environment, { NODE_ENV: 'development' });
  assert.deepEqual(original.expected_ports, [8080]);
  const longCopy = duplicateServiceDefinition({ ...original, name: 'x'.repeat(160) }, ' Copy');
  assert.equal(longCopy.name.length, 160);
  const chineseCopy = duplicateServiceDefinition(
    { ...original, name: '服'.repeat(53) + 'A' },
    ' 副本',
  );
  assert.equal(chineseCopy.name, '服'.repeat(51) + ' 副本');
  assert.equal(Buffer.byteLength(chineseCopy.name), 160);
  const emojiCopy = duplicateServiceDefinition({ ...original, name: '😊'.repeat(40) }, ' 副本');
  assert.equal(emojiCopy.name, '😊'.repeat(38) + ' 副本');
});

test('groupServicesByProject preserves first-seen project order and groups sibling services together', () => {
  const items = [
    { definition: { id: 's1', project_id: 'p1', name: 'frontend' }, current_run: null },
    { definition: { id: 's2', project_id: 'p2', name: 'api' }, current_run: null },
    { definition: { id: 's3', project_id: 'p1', name: 'backend' }, current_run: null },
    { definition: { id: 's4', project_id: 'missing', name: 'orphan' }, current_run: null },
  ];
  const groups = groupServicesByProject(items, [
    { id: 'p1', name: 'Meridian Shop', is_favorite: true },
    { id: 'p2', name: 'Notes API', is_favorite: false },
  ]);
  assert.deepEqual(
    groups.map((g) => ({
      id: g.projectId,
      name: g.projectName,
      favorite: g.favorite,
      names: g.services.map((s) => s.definition.name),
    })),
    [
      { id: 'p1', name: 'Meridian Shop', favorite: true, names: ['frontend', 'backend'] },
      { id: 'p2', name: 'Notes API', favorite: false, names: ['api'] },
      { id: 'missing', name: '已移除的项目', favorite: false, names: ['orphan'] },
    ],
  );
});
