import { readFile, writeFile, readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { architectures, recordBuild, checksumFilename } from '../site/release.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const packageJson = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--target'))
  throw new Error('Usage: node scripts/record-release.mjs [--target <macOS Rust target>]');
const target = args[1];
const arch = target
  ? Object.keys(architectures).find((key) => architectures[key].target === target)
  : process.arch === 'arm64'
    ? 'aarch64'
    : process.arch === 'x64'
      ? 'x86_64'
      : null;
if (process.platform !== 'darwin' || !arch) throw new Error('Unsupported macOS build target');
const output = target ? join(root, 'target', target, 'release') : join(root, 'target/release');
const app = join(output, 'bundle/macos/DevNook.app');
const directory = join(output, 'bundle/dmg');
const filename = `DevNook_${packageJson.version}_${architectures[arch].filenameArch}.dmg`;
const dmg = join(directory, filename);
const current = JSON.parse(await readFile(join(root, 'site/release.json'), 'utf8'));
const binary = join(app, 'Contents/MacOS/devnook-desktop');
const actualArch = execFileSync('/usr/bin/lipo', ['-archs', binary], { encoding: 'utf8' }).trim();
if (actualArch !== architectures[arch].macho) throw new Error('Application architecture mismatch');
for (const [key, expected] of [
  ['CFBundleShortVersionString', packageJson.version],
  ['LSMinimumSystemVersion', '12.0'],
]) {
  const value = execFileSync(
    '/usr/libexec/PlistBuddy',
    ['-c', `Print :${key}`, join(app, 'Contents/Info.plist')],
    { encoding: 'utf8' },
  ).trim();
  if (value !== expected) throw new Error(`Application ${key} mismatch`);
}
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
const manifest = recordBuild(current, packageJson.version, {
  arch,
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
await writeFile(join(directory, checksumFilename(arch)), `${sha256}  ${filename}\n`);
console.log(`Recorded local ${arch} Release candidate, not a public download: ${filename}`);
