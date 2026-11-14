import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const compile = (source) =>
  ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  }).outputText;
const dataUrl = (source) => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const catalogSource = await readFile(new URL('../src/locales/en.ts', import.meta.url), 'utf8');
const catalogUrl = dataUrl(compile(catalogSource));
const { english } = await import(catalogUrl);
const source = await readFile(new URL('../src/i18n.ts', import.meta.url), 'utf8');
const moduleUrl = dataUrl(
  compile(source).replace(/from ['"]\.\/locales\/en['"]/, `from ${JSON.stringify(catalogUrl)}`),
);
const {
  resolveLanguage,
  initializeLanguage,
  changeLanguage,
  languageSnapshot,
  languageStorageKey,
  subscribeLanguage,
  dateLocale,
  t,
} = await import(moduleUrl);

function storage() {
  const values = new Map();
  globalThis.localStorage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
  return values;
}

test('only a primary Chinese language selects Chinese; every other system language selects English', () => {
  for (const locale of ['zh', 'zh-CN', 'zh-Hans-CN', 'zh-Hant-TW', 'zh-HK', 'ZH_cn'])
    assert.equal(resolveLanguage(locale), 'zh', locale);
  for (const locale of [null, undefined, '', 'en', 'en-US', 'ja-JP', 'de-DE', 'fr-FR', 'zhuang', 'en-zh'])
    assert.equal(resolveLanguage(locale), 'en', String(locale));
  assert.equal(resolveLanguage('ja-JP', 'zh-CN'), 'en');
  assert.equal(resolveLanguage('ja-JP', '{invalid}'), 'en');
});

test('manual choice wins over the system language and survives a new initialization', () => {
  const values = storage();
  initializeLanguage('zh-CN');
  assert.equal(languageSnapshot().language, 'zh');
  assert.equal(values.size, 0, 'automatic detection must not become a manual override');
  changeLanguage('en');
  assert.equal(values.get(languageStorageKey), 'en');
  initializeLanguage('zh-CN');
  assert.equal(languageSnapshot().language, 'en');
  changeLanguage('zh');
  initializeLanguage('ja-JP');
  assert.equal(languageSnapshot().language, 'zh');
});

test('unavailable storage preserves automatic selection and immediate manual switching', () => {
  globalThis.localStorage = {
    getItem() { throw Error('unavailable'); },
    setItem() { throw Error('unavailable'); },
  };
  initializeLanguage('zh-Hant-HK');
  assert.equal(languageSnapshot().language, 'zh');
  let changes = 0;
  const unsubscribe = subscribeLanguage(() => changes++);
  changeLanguage('en');
  assert.deepEqual(languageSnapshot(), { language: 'en', storageError: true });
  assert.equal(t('设置'), 'Settings');
  assert.equal(changes, 1);
  unsubscribe();
  changeLanguage('zh');
  assert.equal(changes, 1);
});

test('translations interpolate literal data and already-visible messages switch without remounting', () => {
  storage();
  initializeLanguage('en-GB');
  const name = '中文项目 $& <script> & /tmp/a';
  const message = t('已请求打开 {{0}}', [name]);
  assert.equal(message, `Requested to open ${name}`);
  assert.equal(dateLocale(), 'en-US');
  assert.equal(t('完全自定义的项目名称'), '完全自定义的项目名称');
  assert.equal(t(undefined), '');
  changeLanguage('zh');
  assert.equal(t(message), `已请求打开 ${name}`);
  assert.equal(dateLocale(), 'zh-CN');
});

test('catalog entries are translated and retain exactly the same placeholders', () => {
  const placeholders = (value) => [...value.matchAll(/\{\{\d+\}\}/g)].map(([match]) => match).sort();
  assert.ok(Object.keys(english).length > 680);
  for (const [key, value] of Object.entries(english)) {
    assert.ok(value.length > 0, key);
    assert.ok(!/[\u3400-\u9fff]/.test(value), key);
    assert.deepEqual(placeholders(value), placeholders(key), key);
  }
});

test('startup translation works without Object.hasOwn in older WebViews', () => {
  storage();
  initializeLanguage('en-US');
  const original = Object.hasOwn;
  let text;
  try {
    Object.hasOwn = undefined;
    text = t('设置');
  } finally {
    Object.hasOwn = original;
  }
  assert.equal(text, 'Settings');
});

test('all static application translation calls have catalog entries', async () => {
  const directory = new URL('../src/', import.meta.url);
  for (const file of (await readdir(directory)).filter((file) => /\.(ts|tsx)$/.test(file))) {
    if (file === 'i18n.ts') continue;
    const text = await readFile(new URL(file, directory), 'utf8');
    const tree = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
    function visit(node) {
      if (
        ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 't' &&
        node.arguments[0] && ts.isStringLiteral(node.arguments[0])
      ) assert.ok(Object.hasOwn(english, node.arguments[0].text), `${file}: ${node.arguments[0].text}`);
      ts.forEachChild(node, visit);
    }
    visit(tree);
  }
});
