import { readFile, writeFile, readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { validateRelease } from '../site/template.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const packageJson = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
const app = join(root, 'target/release/bundle/macos/DevNook.app');
const directory = join(root, 'target/release/bundle/dmg');
const filename = `DevNook_${packageJson.version}_aarch64.dmg`;
const dmg = join(directory, filename);
execFileSync('/usr/bin/codesign', ['--verify', '--strict', '--deep', app], { stdio: 'inherit' });
// codesign writes signature details to stderr. Never infer notarization from a signature.
const signatureProbe = spawnSync('/usr/bin/codesign', ['-dv', app], { encoding: 'utf8' });
if (signatureProbe.status !== 0 || !signatureProbe.stderr.includes('Signature=adhoc'))
  throw new Error(
    'This helper records local ad-hoc candidates only; use the documented signed-release process for Developer ID builds.',
  );
async function bytes(path) {
  const info = await stat(path);
  if (!info.isDirectory()) return info.size;
  let total = 0;
  for (const item of await readdir(path)) total += await bytes(join(path, item));
  return total;
}
const sha256 = createHash('sha256')
  .update(await readFile(dmg))
  .digest('hex');
const manifest = validateRelease({
  version: packageJson.version,
  arch: 'aarch64',
  minMacOS: '12.0',
  published: false,
  url: null,
  dmgBytes: (await stat(dmg)).size,
  appBytes: await bytes(app),
  sha256,
  signing: 'ad-hoc',
  notarized: false,
});
await writeFile(join(root, 'site/release.json'), JSON.stringify(manifest, null, 2) + '\n');
await writeFile(join(directory, 'SHA256SUMS'), `${sha256}  ${filename}\n`);
console.log(`Recorded local Release candidate, not a public download: ${filename}`);
