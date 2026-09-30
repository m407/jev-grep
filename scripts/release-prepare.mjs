import { execFileSync } from 'node:child_process';
import { accessSync } from 'node:fs';

export async function prepare(_options, { nextRelease, cwd }) {
  if (nextRelease.version !== process.env.RELEASE_VERSION) {
    throw new Error('Release version changed since planning; refusing to publish mismatched binaries');
  }
  for (const suffix of [
    'x86_64-unknown-linux-gnu.tar.gz',
    'aarch64-apple-darwin.tar.gz',
    'x86_64-pc-windows-msvc.zip',
  ]) {
    accessSync(`${cwd}/dist/jev-grep-v${nextRelease.version}-${suffix}`);
  }
  accessSync(`${cwd}/dist/SHA256SUMS`);
  execFileSync('python3', ['scripts/set-release-version.py', nextRelease.version], {
    cwd,
    stdio: 'inherit',
  });
}
