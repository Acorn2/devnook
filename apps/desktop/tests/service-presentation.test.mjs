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
