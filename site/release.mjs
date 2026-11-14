import { repo } from './content.mjs';

export const architectures = {
  aarch64: {
    target: 'aarch64-apple-darwin',
    macho: 'arm64',
    filenameArch: 'aarch64',
    label: 'Apple Silicon',
  },
  x86_64: {
    target: 'x86_64-apple-darwin',
    macho: 'x86_64',
    filenameArch: 'x64',
    label: 'Intel Mac',
  },
};

export function validateRelease(release) {
  if (!/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(release.version))
    throw new Error('Invalid release version');
  if (!Array.isArray(release.builds) || !release.builds.length || release.builds.length > 2)
    throw new Error('A release requires one or two architecture builds');
  const seen = new Set();
  for (const build of release.builds) {
    if (!Object.hasOwn(architectures, build.arch) || build.minMacOS !== '12.0')
      throw new Error('Unsupported release target');
    if (seen.has(build.arch)) throw new Error('Duplicate release architecture');
    seen.add(build.arch);
    if (typeof build.published !== 'boolean' || typeof build.notarized !== 'boolean')
      throw new Error('Invalid release state');
    if (!['ad-hoc', 'developer-id'].includes(build.signing))
      throw new Error('Unknown signing state');
    if (build.notarized && build.signing !== 'developer-id')
      throw new Error('Notarization requires Developer ID signing');
    const hasArtifact = build.dmgBytes !== null;
    if (
      hasArtifact &&
      (!Number.isSafeInteger(build.dmgBytes) ||
        build.dmgBytes <= 0 ||
        !Number.isSafeInteger(build.appBytes) ||
        build.appBytes <= 0 ||
        !/^[a-f0-9]{64}$/.test(build.sha256))
    )
      throw new Error('Invalid artifact size or checksum');
    if (!hasArtifact && (build.appBytes !== null || build.sha256 !== null))
      throw new Error('Incomplete artifact metadata');
    if (build.published) {
      const expected = `${repo}/releases/download/v${release.version}/DevNook_${release.version}_${architectures[build.arch].filenameArch}.dmg`;
      if (!hasArtifact || build.url !== expected)
        throw new Error('Published releases require an exact version and architecture asset URL');
    } else if (build.url !== null)
      throw new Error('Unpublished artifacts cannot have a download URL');
  }
  return release;
}

export function recordBuild(current, version, build) {
  validateRelease(current);
  const builds =
    current.version === version ? current.builds.filter((b) => b.arch !== build.arch) : [];
  return validateRelease({
    version,
    builds: [...builds, build].sort((a, b) => a.arch.localeCompare(b.arch)),
  });
}

export function checksumFilename(arch) {
  if (!Object.hasOwn(architectures, arch)) throw new Error('Unsupported checksum architecture');
  return arch === 'aarch64' ? 'SHA256SUMS' : 'SHA256SUMS-x86_64';
}
