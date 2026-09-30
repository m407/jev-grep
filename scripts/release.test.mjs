import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { analyzeCommits } from '@semantic-release/commit-analyzer';
import { analysisPlugins } from '../release.config.js';
import { prepare } from './release-prepare.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const versionScript = join(root, 'scripts/set-release-version.py');
const logger = { log() {} };

for (const [message, expected] of [
  ['fix: handle timeout', 'patch'],
  ['perf: reuse connections', 'patch'],
  ['feat(cli): add output format', 'minor'],
  ['feat(api)!: change response format', 'major'],
  ['fix: change response\n\nBREAKING CHANGE: remove old fields', 'major'],
  ['docs: explain installation', null],
  ['chore(release): 1.2.3 [skip ci]', null],
]) {
  test(`release classification: ${message.split('\n')[0]}`, async () => {
    const result = await analyzeCommits(analysisPlugins[0][1], {
      cwd: root, commits: [{ hash: 'test', message }], logger,
    });
    assert.equal(result, expected);
  });
}

test('highest release type wins across new commits', async () => {
  const result = await analyzeCommits(analysisPlugins[0][1], {
    cwd: root,
    commits: ['fix: handle timeout', 'feat: support JSON', 'docs: explain flags']
      .map((message) => ({ message })),
    logger,
  });
  assert.equal(result, 'minor');
});

function fixture(t) {
  const dir = mkdtempSync(join(process.env.RUNNER_TEMP || tmpdir(), 'jev-release-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const file of ['Cargo.toml', 'Cargo.lock']) {
    copyFileSync(join(root, file), join(dir, file));
  }
  return dir;
}

test('version update preserves all other manifest and lockfile content', (t) => {
  const dir = fixture(t);
  const before = Object.fromEntries(['Cargo.toml', 'Cargo.lock']
    .map((file) => [file, readFileSync(join(dir, file), 'utf8')]));
  const oldVersion = before['Cargo.toml'].match(/^version = "([^"]+)"/m)[1];
  execFileSync('python3', [versionScript, '2.3.4'], { cwd: dir });
  for (const file of Object.keys(before)) {
    const prefix = file === 'Cargo.toml'
      ? 'name = "jev-grep"\n'
      : '[[package]]\nname = "jev-grep"\n';
    assert.equal(readFileSync(join(dir, file), 'utf8'),
      before[file].replace(`${prefix}version = "${oldVersion}"`, `${prefix}version = "2.3.4"`));
  }
  // Reapplying a calculated version is harmless.
  execFileSync('python3', [versionScript, '2.3.4'], { cwd: dir });
});

test('invalid version or inconsistent lockfile fails without partial writes', (t) => {
  const dir = fixture(t);
  const manifest = readFileSync(join(dir, 'Cargo.toml'), 'utf8');
  const originalLock = readFileSync(join(dir, 'Cargo.lock'), 'utf8');
  let result = spawnSync('python3', [versionScript, 'bad\nversion'], { cwd: dir });
  assert.notEqual(result.status, 0);
  assert.equal(readFileSync(join(dir, 'Cargo.toml'), 'utf8'), manifest);
  assert.equal(readFileSync(join(dir, 'Cargo.lock'), 'utf8'), originalLock);
  const badLock = originalLock.replace(/(name = "jev-grep"\nversion = ")[^"]+/, (_match, prefix) => `${prefix}99.0.0`);
  writeFileSync(join(dir, 'Cargo.lock'), badLock);
  result = spawnSync('python3', [versionScript, '2.3.4'], { cwd: dir });
  assert.notEqual(result.status, 0);
  assert.equal(readFileSync(join(dir, 'Cargo.toml'), 'utf8'), manifest);
  assert.equal(readFileSync(join(dir, 'Cargo.lock'), 'utf8'), badLock);
});

test('prepare blocks version drift and missing platform assets before changing Cargo files', async (t) => {
  const dir = fixture(t);
  const oldEnv = process.env.RELEASE_VERSION;
  t.after(() => {
    if (oldEnv === undefined) delete process.env.RELEASE_VERSION;
    else process.env.RELEASE_VERSION = oldEnv;
  });
  process.env.RELEASE_VERSION = '2.3.4';
  const before = readFileSync(join(dir, 'Cargo.toml'), 'utf8');
  await assert.rejects(prepare({}, { cwd: dir, nextRelease: { version: '2.4.0' } }), /version changed/);
  await assert.rejects(prepare({}, { cwd: dir, nextRelease: { version: '2.3.4' } }), /ENOENT/);
  assert.equal(readFileSync(join(dir, 'Cargo.toml'), 'utf8'), before);
  mkdirSync(join(dir, 'dist'));
  for (const suffix of ['x86_64-unknown-linux-gnu.tar.gz', 'aarch64-apple-darwin.tar.gz', 'x86_64-pc-windows-msvc.zip']) {
    writeFileSync(join(dir, `dist/jev-grep-v2.3.4-${suffix}`), 'fixture');
  }
  writeFileSync(join(dir, 'dist/SHA256SUMS'), 'fixture');
  mkdirSync(join(dir, 'scripts'));
  copyFileSync(versionScript, join(dir, 'scripts/set-release-version.py'));
  await prepare({}, { cwd: dir, nextRelease: { version: '2.3.4' } });
  assert.match(readFileSync(join(dir, 'Cargo.toml'), 'utf8'), /^version = "2.3.4"$/m);
});

test('semantic-release plans and tags the same Cargo version against a local remote', async (t) => {
  const dir = fixture(t);
  const remote = join(dir, 'remote.git');
  const repo = join(dir, 'repo');
  const git = (...args) => execFileSync('git', args, { cwd: dir, stdio: 'pipe' }).toString().trim();
  git('init', '--bare', '--initial-branch=main', remote);
  git('clone', remote, repo);
  git('-C', repo, 'config', 'user.name', 'Release Test');
  git('-C', repo, 'config', 'user.email', 'release-test@example.invalid');
  for (const file of ['Cargo.toml', 'Cargo.lock']) {
    copyFileSync(join(dir, file), join(repo, file));
  }
  execFileSync('python3', [versionScript, '0.1.0'], { cwd: repo });
  git('-C', repo, 'add', '.');
  git('-C', repo, 'commit', '-m', 'feat: initial release');
  git('-C', repo, 'tag', 'v0.1.0');
  git('-C', repo, 'commit', '--allow-empty', '-m', 'fix: handle timeout');
  git('-C', repo, 'push', 'origin', 'main', '--tags');

  const options = {
    ci: false,
    branches: ['main'],
    repositoryUrl: `file://${remote}`,
    tagFormat: 'v${version}',
    // Resolve from the real tooling installation, not the temporary repository.
    plugins: analysisPlugins.map(([name, config]) => [fileURLToPath(import.meta.resolve(name)), config]),
  };
  const env = {
    PATH: process.env.PATH,
    HOME: process.env.HOME,
    GIT_AUTHOR_NAME: 'Release Test', GIT_AUTHOR_EMAIL: 'release-test@example.invalid',
    GIT_COMMITTER_NAME: 'Release Test', GIT_COMMITTER_EMAIL: 'release-test@example.invalid',
  };
  // semantic-release hooks stdout; isolate it from node:test's reporting protocol.
  function runRelease(options) {
    const output = execFileSync(process.execPath, ['--input-type=module', '-e', `
      import semanticRelease from ${JSON.stringify(import.meta.resolve('semantic-release'))};
      const result = await semanticRelease(${JSON.stringify(options)});
      console.log('RESULT=' + JSON.stringify(result));
    `], { cwd: repo, env, encoding: 'utf8' });
    return JSON.parse(output.split('\n').find((line) => line.startsWith('RESULT=')).slice(7));
  }
  const planned = runRelease({ ...options, dryRun: true });
  assert.equal(planned.nextRelease.version, '0.1.1');
  assert.match(planned.nextRelease.notes, /Bug Fixes/);
  assert.match(planned.nextRelease.notes, /handle timeout/);
  assert.equal(git('-C', repo, 'tag', '--list', 'v0.1.1'), '');

  mkdirSync(join(repo, 'dist'));
  for (const suffix of ['x86_64-unknown-linux-gnu.tar.gz', 'aarch64-apple-darwin.tar.gz', 'x86_64-pc-windows-msvc.zip']) {
    writeFileSync(join(repo, `dist/jev-grep-v0.1.1-${suffix}`), 'fixture');
  }
  writeFileSync(join(repo, 'dist/SHA256SUMS'), 'fixture');
  mkdirSync(join(repo, 'scripts'));
  copyFileSync(versionScript, join(repo, 'scripts/set-release-version.py'));
  env.RELEASE_VERSION = planned.nextRelease.version;
  const published = runRelease({
    ...options,
    plugins: [
      ...options.plugins,
      join(root, 'scripts/release-prepare.mjs'),
      [fileURLToPath(import.meta.resolve('@semantic-release/git')), {
        assets: ['Cargo.toml', 'Cargo.lock'],
        message: 'chore(release): ${nextRelease.version} [skip ci]',
      }],
    ],
  });
  assert.equal(published.nextRelease.version, planned.nextRelease.version);
  assert.match(git('-C', repo, 'show', 'v0.1.1:Cargo.toml'), /^version = "0.1.1"$/m);
  assert.match(git('-C', repo, 'show', 'v0.1.1:Cargo.lock'), /name = "jev-grep"\nversion = "0.1.1"/);
  assert.equal(git('-C', remote, 'rev-parse', 'refs/heads/main'), git('-C', repo, 'rev-parse', 'v0.1.1'));
});
