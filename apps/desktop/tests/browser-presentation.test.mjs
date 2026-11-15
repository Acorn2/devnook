import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/BrowserPresentation.ts', import.meta.url), 'utf8');
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
});
const { preferredHttpAddresses, httpsAlternative, browserHttpChoices, browserShortcut } =
  await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
const address = (url, changes = {}) => ({
  url,
  pid: 41,
  fingerprint: 'current-instance',
  ...changes,
});

test('page endpoints take priority; unknown endpoints stay accessible before confirmed APIs', () => {
  const api = address('http://127.0.0.1:3000/', { web_ui: false });
  const legacy = address('http://127.0.0.1:5173/');
  const page = address('http://127.0.0.1:4320/', { web_ui: true });
  assert.deepEqual(browserHttpChoices([api, legacy]), [legacy]);
  assert.deepEqual(browserHttpChoices([api, legacy, page]), [page]);
  assert.deepEqual(
    browserHttpChoices([address(api.url, { web_ui: 'true' })]).map((a) => a.web_ui),
    ['true'],
  );
});

test('custom Node pages keep primary open, confirmed API roots hide it, and uncertainty never hides open', () => {
  assert.equal(browserShortcut(address('http://127.0.0.1:4318/', { web_ui: true })), 'page');
  assert.equal(browserShortcut(address('http://127.0.0.1:3847/', { web_ui: false })), 'none');
  for (const hint of [undefined, null]) {
    assert.equal(browserShortcut(address('http://127.0.0.1:4318/', { web_ui: hint })), 'try');
  }
  assert.equal(browserShortcut(), 'none');
});

test('a known page keeps its own PID and endpoint even when an unknown IPv4 listener uses the same port', () => {
  const api = address('http://127.0.0.1:3000/', { web_ui: false });
  const page = address('http://[::1]:3000/', {
    pid: 42,
    fingerprint: 'page-instance',
    web_ui: true,
  });
  assert.equal(browserHttpChoices([api, page])[0], page);
});

test('default choices use only original HTTP candidates, preferring IPv4 and one choice per actual port', () => {
  const choices = [
    address('https://127.0.0.1:3000/'),
    address('http://[::1]:3000/'),
    address('http://127.0.0.1:3000/'),
    address('http://127.0.0.1:3001/'),
    address('http://127.0.0.1:3000/'),
    address('http://invalid host/'),
  ];
  const before = structuredClone(choices);
  const http = preferredHttpAddresses(choices);
  assert.deepEqual(
    http.map((a) => a.url),
    ['http://127.0.0.1:3000/', 'http://127.0.0.1:3001/'],
  );
  assert.equal(http[0], choices[2]);
  assert.deepEqual(choices, before);
  assert.deepEqual(preferredHttpAddresses([address('https://127.0.0.1:3000/')]), []);
});

test('IPv6-only listeners remain usable and no HTTPS endpoint is invented', () => {
  const choices = [address('http://[::1]:3000/')];
  assert.equal(preferredHttpAddresses(choices)[0], choices[0]);
  assert.equal(httpsAlternative(choices, choices[0]), undefined);
  assert.equal(httpsAlternative(choices), undefined);
});

test('explicit HTTPS must match the selected host, port, PID and current fingerprint', () => {
  const selected = address('http://127.0.0.1:3001/');
  const correct = address('https://127.0.0.1:3001/');
  const choices = [
    address('https://127.0.0.1:3000/'),
    address('https://[::1]:3001/'),
    address(correct.url, { pid: 42 }),
    address(correct.url, { fingerprint: 'old-instance' }),
    correct,
  ];
  assert.equal(httpsAlternative(choices, selected), correct);
  assert.equal(httpsAlternative(choices.slice(0, -1), selected), undefined);
});

test('HTTPS alternatives preserve the TCP port when URL serialization omits standard ports', () => {
  for (const [httpUrl, httpsUrl] of [
    ['http://127.0.0.1/', 'https://127.0.0.1:80/'],
    ['http://127.0.0.1:443/', 'https://127.0.0.1/'],
  ]) {
    const https = address(httpsUrl);
    assert.equal(httpsAlternative([https], address(httpUrl)), https);
  }
});
