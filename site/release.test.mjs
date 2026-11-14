import test from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from './template.mjs';
import { validateRelease, recordBuild, checksumFilename, architectures } from './release.mjs';

const build = {
  arch: 'aarch64',
  minMacOS: '12.0',
  published: false,
  url: null,
  dmgBytes: 5_000_000,
  appBytes: 10_000_000,
  sha256: 'a'.repeat(64),
  signing: 'ad-hoc',
  notarized: false,
};
const release = { version: '0.1.0', builds: [build] };
const publicBuild = (arch) => ({
  ...build,
  arch,
  published: true,
  url: `https://github.com/Acorn2/devnook/releases/download/v0.1.0/DevNook_0.1.0_${architectures[arch].filenameArch}.dmg`,
});

test('unpublished installers offer source builds in both languages', () => {
  for (const locale of ['zh', 'en']) {
    const html = renderPage(locale, release);
    assert.doesNotMatch(html, /href="[^"]*releases\/download\//);
    assert.doesNotMatch(html, / download>/);
    assert.match(html, /href="https:\/\/github.com\/Acorn2\/devnook\/blob\/main\/README/);
    assert.match(html, /5\.00 MB/);
    assert.match(html, locale === 'zh' ? /尚未公开发布/ : /not published yet/);
    assert.match(
      html,
      locale === 'zh' ? /尚未完成 Developer ID/ : /has not completed Developer ID/,
    );
  }
});

test('both architectures have distinct, correctly labeled downloads and signing status', () => {
  const builds = ['aarch64', 'x86_64'].map(publicBuild);
  for (const locale of ['zh', 'en']) {
    const html = renderPage(locale, { ...release, builds });
    for (const candidate of builds) assert.ok(html.includes(`href="${candidate.url}" download`));
    assert.match(html, /Apple Silicon DMG/);
    assert.match(html, /Intel Mac DMG/);
    assert.equal((html.match(/ download>/g) || []).length, 2);
    assert.doesNotMatch(html, /Developer ID signed and Apple notarized/);
  }
});

test('a local Intel build never hides the published ARM installer or exposes an Intel URL', () => {
  const arm = publicBuild('aarch64');
  const intel = { ...build, arch: 'x86_64', sha256: 'b'.repeat(64) };
  const html = renderPage('en', { ...release, builds: [arm, intel] });
  assert.ok(html.includes(`href="${arm.url}" download`));
  assert.doesNotMatch(html, /href="[^"]*x64\.dmg/);
  assert.match(html, /Intel Mac/);
  assert.match(html, /not published yet/);
});

test('cross-repository, cross-architecture, incomplete and contradictory builds are rejected', () => {
  for (const changes of [
    { published: true },
    { published: true, url: 'https://example.com/app.dmg' },
    { published: true, url: 'https://github.com/Acorn2/other/releases/download/v0.1.0/app.dmg' },
    { ...publicBuild('aarch64'), arch: 'x86_64' },
    { sha256: null },
    { dmgBytes: -1 },
    { dmgBytes: null },
    { notarized: true },
    { arch: 'universal' },
    { arch: 'toString' },
    { minMacOS: '11.0' },
    { url: 'https://github.com/Acorn2/devnook/releases' },
  ])
    assert.throws(() => renderPage('en', { ...release, builds: [{ ...build, ...changes }] }));
});

test('missing, empty or duplicate architecture lists cannot generate downloads', () => {
  for (const builds of [undefined, [], [build, build], [build, build, build]])
    assert.throws(() => validateRelease({ ...release, builds }));
});

test('recording Intel preserves public ARM metadata without mutating it', () => {
  const arm = publicBuild('aarch64');
  const current = { ...release, builds: [arm] };
  const intel = { ...build, arch: 'x86_64' };
  const result = recordBuild(current, '0.1.0', intel);
  assert.deepEqual(result.builds, [arm, intel]);
  assert.deepEqual(current.builds, [arm]);
  const replaced = recordBuild(result, '0.1.0', { ...intel, dmgBytes: 6_000_000 });
  assert.deepEqual(replaced.builds[0], arm);
  assert.equal(replaced.builds[1].dmgBytes, 6_000_000);
});

test('a new version does not carry forward assets built for an older source version', () => {
  const current = { ...release, builds: [publicBuild('aarch64'), publicBuild('x86_64')] };
  const next = recordBuild(current, '0.2.0', build);
  assert.deepEqual(next, { version: '0.2.0', builds: [build] });
  assert.equal(current.builds.length, 2);
});

test('checksum names retain the existing ARM checksum asset', () => {
  assert.equal(checksumFilename('aarch64'), 'SHA256SUMS');
  assert.equal(checksumFilename('x86_64'), 'SHA256SUMS-x86_64');
  assert.throws(() => checksumFilename('universal'));
});
