import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

// Run the pure presentation helpers on the same Node 22 used in CI, without
// adding a test runtime or executing native APIs.
const source = await readFile(new URL('../src/PortPresentation.ts', import.meta.url), 'utf8');
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
});
const { filterPortBindings, portRows, conflictingBindings, portRuntime } = await import(
  `data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
);
const all = {
  query: '',
  project: 'ALL',
  family: 'ALL',
  runtime: 'ALL',
  kind: 'ALL',
  source: 'ALL',
};
const binding = (changes = {}) => ({
  pid: 11,
  port: 3000,
  family: 'IPv4',
  address: '127.0.0.1',
  name: 'node',
  project_id: 'alpha',
  command: 'node api.js',
  cwd: '/projects/alpha',
  source: 'OBSERVED',
  service_id: null,
  process_kind: 'SERVICE',
  ...changes,
});
const bindings = [
  binding(),
  binding({ family: 'IPv6', address: '::1' }),
  binding({
    pid: 12,
    port: 3000,
    name: 'java',
    project_id: 'beta',
    source: 'DEVNOOK',
    service_id: 'java-api',
  }),
  binding({
    pid: 13,
    port: 5000,
    name: 'python3.12',
    process_kind: 'DEVELOPMENT_TOOL',
    project_id: null,
  }),
  binding({
    pid: 14,
    port: 8000,
    name: 'custom-api',
    process_kind: 'UNKNOWN',
    project_id: 'alpha',
  }),
];
const projects = [
  { id: 'alpha', name: '项目甲' },
  { id: 'beta', name: '项目乙' },
];

test('combined filters preserve the complete snapshot denominator and merge only PID + port', () => {
  const total = portRows(bindings).length;
  assert.equal(total, 4);
  const visible = filterPortBindings(bindings, projects, {
    ...all,
    query: '项目甲',
    project: 'alpha',
    family: 'IPv6',
    runtime: 'NODE',
    kind: 'SERVICE',
    source: 'OBSERVED',
  });
  assert.equal(portRows(visible).length, 1);
  assert.equal(visible[0].family, 'IPv6');
  assert.equal(portRows(bindings).length, total);
  assert.equal(
    portRows(filterPortBindings(bindings, projects, { ...all, query: 'missing' })).length,
    0,
  );
});

test('unknown purpose and externally launched project processes remain distinct from managed services', () => {
  assert.deepEqual(
    filterPortBindings(bindings, projects, { ...all, query: '未关联项目' }).map((p) => p.pid),
    [13],
  );
  assert.deepEqual(
    filterPortBindings(bindings, projects, { ...all, query: 'Unassigned project' }).map((p) => p.pid),
    [13],
  );
  assert.deepEqual(
    filterPortBindings(bindings, projects, { ...all, source: 'DEVNOOK' }).map((p) => p.pid),
    [12],
  );
  assert.deepEqual(
    filterPortBindings(bindings, projects, { ...all, kind: 'UNKNOWN' }).map((p) => p.pid),
    [14],
  );
  assert.deepEqual(
    filterPortBindings(bindings, projects, { ...all, project: 'EXTERNAL' }).map((p) => p.pid),
    [13],
  );
  assert.equal(filterPortBindings(bindings, projects, { ...all, runtime: 'OTHER' })[0].pid, 14);
});

test('runtime grouping uses the reported executable name without guessing a native app language', () => {
  for (const [name, runtime] of [
    ['node', 'NODE'],
    ['Python3.14', 'PYTHON'],
    ['java', 'JAVA'],
    ['dotnet', 'DOTNET'],
    ['php-fpm', 'PHP'],
    ['ruby3.4', 'RUBY'],
    ['bun', 'BUN'],
    ['deno', 'DENO'],
    ['node-helper', 'OTHER'],
    ['my-go-service', 'OTHER'],
  ]) {
    assert.equal(portRuntime(name), runtime);
  }
});

test('conflicts include other managed and external PIDs, and never claim external ownership from a matching service ID', () => {
  const before = structuredClone(bindings);
  const conflicts = conflictingBindings(bindings, [3000], 'java-api');
  assert.deepEqual(
    portRows(conflicts).map((p) => p.pid),
    [11],
  );
  const external = binding({ service_id: 'java-api' });
  assert.equal(conflictingBindings([external], [3000], 'java-api').length, 1);
  assert.equal(conflictingBindings(bindings, [3000], 'other-api').length, 3);
  assert.deepEqual(bindings, before);
});
