import { appendFileSync, readFileSync, readdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const RELEASE_BRANCH = 'release-please--branches--main';
const PENDING = 'autorelease: pending';
const TAGGED = 'autorelease: tagged';
const SNOOZE = 'autorelease: snooze';
const RELEASE_FILES = ['Cargo.toml', 'Cargo.lock', '.release-please-manifest.json', 'CHANGELOG.md'];
const SHA = /^[a-f0-9]{40}$/;
const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

class ReleaseError extends Error {}

function requireValue(condition, message) {
  if (!condition) throw new ReleaseError(message);
}

export function createGitHub({ token, fetchImpl = fetch, apiUrl = 'https://api.github.com' }) {
  return async (method, path, body, { binary = false, upload = false, optional = false } = {}) => {
    requireValue(token, 'GITHUB_TOKEN is required');
    const url = path.startsWith('https://') ? path : `${apiUrl}${path}`;
    // Upload URLs are constructed locally, never taken from PR content.
    requireValue(
      url.startsWith(`${apiUrl}/`) || url.startsWith('https://uploads.github.com/'),
      'Unexpected GitHub API origin',
    );
    const response = await fetchImpl(url, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: binary ? 'application/octet-stream' : 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        ...(body === undefined
          ? {}
          : { 'Content-Type': upload ? 'application/octet-stream' : 'application/json' }),
      },
      ...(body === undefined ? {} : { body: upload ? body : JSON.stringify(body) }),
    });
    if (optional && response.status === 404) return null;
    // Do not print response bodies or request headers (which can contain credentials).
    requireValue(response.ok, `GitHub ${method} failed (${response.status})`);
    if (response.status === 204) return null;
    return binary ? Buffer.from(await response.arrayBuffer()) : response.json();
  };
}

function repoPath(repository) {
  requireValue(/^[\w.-]+\/[\w.-]+$/.test(repository ?? ''), 'Invalid GITHUB_REPOSITORY');
  return `/repos/${repository}`;
}

async function list(api, path) {
  const results = [];
  for (let page = 1; ; page++) {
    const items = await api(
      'GET',
      `${path}${path.includes('?') ? '&' : '?'}per_page=100&page=${page}`,
    );
    requireValue(Array.isArray(items), 'Invalid GitHub list response');
    results.push(...items);
    if (items.length < 100) return results;
  }
}

function labels(pr) {
  return (pr.labels ?? []).map((label) => label.name);
}

function validateMetadata(pr, repository) {
  requireValue(Number.isSafeInteger(pr?.number) && pr.number > 0, 'Invalid PR number');
  requireValue(typeof pr.title === 'string' && pr.title.trim(), 'Invalid PR title');
  requireValue(
    pr.base?.repo?.full_name === repository && pr.base.ref === 'main',
    'Invalid PR base',
  );
  requireValue(SHA.test(pr.base.sha) && SHA.test(pr.head?.sha), 'Invalid PR commit metadata');
}

export function releaseBot(slug) {
  requireValue(/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug ?? ''), 'RELEASE_APP_SLUG is required');
  requireValue(!['github-actions', 'dependabot'].includes(slug), 'Dedicated release App required');
  return `${slug}[bot]`;
}

export function validateReleasePR(
  pr,
  repository,
  { merged = false, published = false, appSlug } = {},
) {
  validateMetadata(pr, repository);
  requireValue(pr.user?.login === releaseBot(appSlug), 'Invalid release PR author');
  requireValue(pr.head?.repo?.full_name === repository, 'Invalid release PR repository');
  requireValue(pr.head.ref === RELEASE_BRANCH, 'Invalid release PR branch');
  requireValue(!pr.draft, 'Release PR is a draft');
  const names = labels(pr);
  requireValue(!names.includes(SNOOZE), 'Release PR is snoozed');
  requireValue(
    published
      ? names.includes(PENDING) || names.includes(TAGGED)
      : names.includes(PENDING) && !names.includes(TAGGED),
    'Invalid release PR labels',
  );
  if (merged) {
    requireValue(
      pr.state === 'closed' && pr.merged_at && SHA.test(pr.merge_commit_sha),
      'Release PR is not merged',
    );
  } else {
    requireValue(pr.state === 'open' && !pr.merged_at, 'Release PR is not open');
  }
  return pr;
}

function contextOutputs(sha, pr) {
  return {
    check_sha: sha,
    pr_number: pr ? String(pr.number) : '',
    pr_title: pr?.title ?? '',
    base_sha: pr?.base.sha ?? '',
    head_sha: pr?.head.sha ?? '',
  };
}

function localVersions(cwd) {
  return Object.fromEntries(
    RELEASE_FILES.slice(0, 3).map((name) => [name, readFileSync(join(cwd, name), 'utf8')]),
  );
}

export async function context({
  env,
  event,
  api,
  cwd = process.cwd(),
  readVersions = localVersions,
}) {
  const path = repoPath(env.GITHUB_REPOSITORY);
  requireValue(SHA.test(env.GITHUB_SHA), 'Invalid GITHUB_SHA');
  requireValue(event.repository?.full_name === env.GITHUB_REPOSITORY, 'Invalid event repository');
  if (env.GITHUB_EVENT_NAME === 'pull_request') {
    const pr = event.pull_request;
    validateMetadata(pr, env.GITHUB_REPOSITORY);
    requireValue(
      event.number === pr.number && env.GITHUB_REF === `refs/pull/${pr.number}/merge`,
      'Invalid PR event ref',
    );
    return contextOutputs(env.GITHUB_SHA, pr);
  }
  if (env.GITHUB_EVENT_NAME === 'workflow_dispatch') {
    const {
      release_pr = '',
      release_sha = '',
      verify_builds = false,
      ...other
    } = event.inputs ?? {};
    requireValue(Object.keys(other).length === 0, 'Unexpected dispatch inputs');
    requireValue(
      [true, false, 'true', 'false'].includes(verify_builds),
      'Invalid verify_builds input',
    );
    if (verify_builds === true || verify_builds === 'true') {
      requireValue(
        release_pr === '' && release_sha === '',
        'Build verification cannot include release inputs',
      );
      requireValue(/^refs\/heads\/.+/.test(env.GITHUB_REF), 'Build verification requires a branch');
      return { ...contextOutputs(env.GITHUB_SHA), version: parseVersions(readVersions(cwd)) };
    }
    if (release_pr !== '' || release_sha !== '') {
      requireValue(
        env.GITHUB_REF === `refs/heads/${RELEASE_BRANCH}`,
        'Invalid validation dispatch branch',
      );
      requireValue(
        /^[1-9]\d*$/.test(release_pr) && SHA.test(release_sha),
        'Invalid validation dispatch inputs',
      );
      const pr = validateReleasePR(
        await api('GET', `${path}/pulls/${release_pr}`),
        env.GITHUB_REPOSITORY,
        { appSlug: env.RELEASE_APP_SLUG },
      );
      requireValue(String(pr.number) === release_pr, 'Dispatch PR number mismatch');
      requireValue(
        pr.head.sha === release_sha && release_sha === env.GITHUB_SHA,
        'Stale release PR head',
      );
      return contextOutputs(release_sha, pr);
    }
  } else {
    requireValue(env.GITHUB_EVENT_NAME === 'push', 'Unsupported release event');
  }
  requireValue(env.GITHUB_REF === 'refs/heads/main', 'Release orchestration requires main');
  return contextOutputs(env.GITHUB_SHA);
}

export function parseVersions(files) {
  let output;
  try {
    output = execFileSync(
      'python3',
      [fileURLToPath(new URL('./release-version.py', import.meta.url))],
      {
        input: JSON.stringify(files),
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );
  } catch {
    throw new ReleaseError(
      'Cannot validate Cargo/release metadata: require Python 3.11+ and three matching versions',
    );
  }
  const version = output.trim();
  requireValue(VERSION.test(version), 'Invalid release version');
  return version;
}

async function candidate(api, repository, pr, parse, appSlug) {
  validateReleasePR(pr, repository, { merged: true, published: true, appSlug });
  const path = repoPath(repository);
  const changed = await list(api, `${path}/pulls/${pr.number}/files`);
  requireValue(
    changed.length > 0 &&
      changed.every(
        (file) =>
          RELEASE_FILES.includes(file.filename) &&
          (!file.previous_filename || RELEASE_FILES.includes(file.previous_filename)),
      ),
    'Unexpected release PR files',
  );
  const sha = pr.merge_commit_sha;
  const comparison = await api('GET', `${path}/compare/${sha}...main`);
  requireValue(
    ['ahead', 'identical'].includes(comparison.status) && comparison.merge_base_commit?.sha === sha,
    'Release merge SHA is not an ancestor of main',
  );
  const files = {};
  for (const file of RELEASE_FILES.slice(0, 3)) {
    const content = await api('GET', `${path}/contents/${file}?ref=${sha}`);
    requireValue(
      content.encoding === 'base64' && content.type === 'file',
      'Invalid release metadata contents',
    );
    files[file] = Buffer.from(content.content, 'base64').toString('utf8');
  }
  return { release_sha: sha, version: parse(files), release_pr_number: String(pr.number) };
}

export function assetNames(version) {
  requireValue(VERSION.test(version), 'Invalid release version');
  return [
    `jev-grep-v${version}-x86_64-unknown-linux-gnu.tar.gz`,
    `jev-grep-v${version}-aarch64-apple-darwin.tar.gz`,
    `jev-grep-v${version}-x86_64-pc-windows-msvc.zip`,
    'SHA256SUMS',
  ];
}

export function validateAssets(assets, version) {
  const expected = assetNames(version);
  requireValue(
    Object.keys(assets).sort().join('\n') === [...expected].sort().join('\n'),
    'Expected exactly three platform archives and SHA256SUMS',
  );
  const checksums = new Map();
  for (const line of assets.SHA256SUMS.toString('utf8').trim().split('\n')) {
    const match = /^([a-f0-9]{64}) [ *]([^\r\n]+)$/.exec(line);
    requireValue(match && !checksums.has(match[2]), 'Invalid or duplicate checksum entry');
    checksums.set(match[2], match[1]);
  }
  requireValue(checksums.size === 3, 'Expected exactly three archive checksums');
  for (const name of expected.slice(0, 3)) {
    requireValue(
      assets[name].length > 0 &&
        checksums.get(name) === createHash('sha256').update(assets[name]).digest('hex'),
      `Checksum mismatch: ${name}`,
    );
  }
}

function localAssets(cwd, version) {
  const dir = join(cwd, 'dist');
  const assets = Object.fromEntries(
    readdirSync(dir).map((name) => [name, readFileSync(join(dir, name))]),
  );
  validateAssets(assets, version);
  return assets;
}

async function tagSha(api, path, tag) {
  const ref = await api('GET', `${path}/git/ref/tags/${tag}`, undefined, { optional: true });
  if (!ref) return null;
  let object = ref.object;
  for (let depth = 0; object.type === 'tag' && depth < 5; depth++) {
    object = (await api('GET', `${path}/git/tags/${object.sha}`)).object;
  }
  requireValue(object.type === 'commit' && SHA.test(object.sha), 'Invalid release tag target');
  return object.sha;
}

async function findRelease(api, path, tag) {
  const matches = (await list(api, `${path}/releases`)).filter(
    (release) => release.tag_name === tag,
  );
  requireValue(matches.length <= 1, 'Duplicate releases for tag');
  return matches[0];
}

async function verifyRemoteAssets(api, path, release, version, expectedAssets) {
  const metadata = await list(api, `${path}/releases/${release.id}/assets`);
  const names = metadata.map((asset) => asset.name).sort();
  requireValue(
    names.join('\n') === assetNames(version).sort().join('\n') &&
      metadata.every((asset) => asset.state === 'uploaded'),
    'Incomplete release assets',
  );
  const assets = {};
  for (const asset of metadata) {
    const bytes = await api('GET', `${path}/releases/assets/${asset.id}`, undefined, {
      binary: true,
    });
    requireValue(bytes.length === asset.size, 'Release asset size mismatch');
    requireValue(
      !expectedAssets || bytes.equals(expectedAssets[asset.name]),
      'Uploaded release asset differs from build',
    );
    assets[asset.name] = bytes;
  }
  validateAssets(assets, version);
}

async function markPublished(api, path, pr) {
  // Add first: interruption leaves pending+tagged, which prepare can repair.
  await api('POST', `${path}/issues/${pr.number}/labels`, { labels: [TAGGED] });
  await api(
    'DELETE',
    `${path}/issues/${pr.number}/labels/${encodeURIComponent(PENDING)}`,
    undefined,
    { optional: true },
  );
}

async function dispatch(api, path, ref, inputs = {}) {
  await api('POST', `${path}/actions/workflows/release.yml/dispatches`, { ref, inputs });
}

export async function createReleasePRs({ repository, token }) {
  requireValue(token, 'RELEASE_PR_TOKEN is required');
  const { GitHub, Manifest } = await import('release-please');
  const [owner, repo] = repository.split('/');
  const github = await GitHub.create({ owner, repo, token });
  const manifest = await Manifest.fromManifest(github, 'main');
  // Publication is exclusively owned by publish(), after the platform builds.
  await manifest.createPullRequests();
}

export async function prepare({ env, api, createPRs = createReleasePRs, parse = parseVersions }) {
  requireValue(env.GITHUB_REF === 'refs/heads/main', 'Release preparation requires main');
  const repository = env.GITHUB_REPOSITORY;
  const path = repoPath(repository);
  const issues = await list(
    api,
    `${path}/issues?state=closed&labels=${encodeURIComponent(PENDING)}`,
  );
  const pending = [];
  for (const issue of issues.filter((issue) => issue.pull_request)) {
    const pr = await api('GET', `${path}/pulls/${issue.number}`);
    if (pr.merged_at) pending.push(pr);
  }
  pending.sort((a, b) => a.merged_at.localeCompare(b.merged_at) || a.number - b.number);
  for (const pr of pending) {
    const release = await candidate(api, repository, pr, parse, env.RELEASE_APP_SLUG);
    const tag = `v${release.version}`;
    const target = await tagSha(api, path, tag);
    requireValue(!target || target === release.release_sha, 'Release tag SHA mismatch');
    const existing = await findRelease(api, path, tag);
    if (existing && !existing.draft) {
      requireValue(target === release.release_sha, 'Published release tag is missing');
      await verifyRemoteAssets(api, path, existing, release.version);
      await markPublished(api, path, pr);
      continue;
    }
    return release;
  }
  releaseBot(env.RELEASE_APP_SLUG);
  requireValue(env.RELEASE_PR_TOKEN, 'RELEASE_PR_TOKEN is required');
  requireValue(
    env.RELEASE_APP_TOKEN_SLUG === env.RELEASE_APP_SLUG,
    'Release token App does not match RELEASE_APP_SLUG',
  );
  await createPRs({ repository, token: env.RELEASE_PR_TOKEN });
  // createPullRequests returns [] for an unchanged PR; discover it independently.
  const open = await list(
    api,
    `${path}/pulls?state=open&base=main&head=${encodeURIComponent(`${repository.split('/')[0]}:${RELEASE_BRANCH}`)}`,
  );
  requireValue(open.length <= 1, 'Multiple open release PRs');
  if (open.length) {
    const pr = validateReleasePR(await api('GET', `${path}/pulls/${open[0].number}`), repository, {
      appSlug: env.RELEASE_APP_SLUG,
    });
    // Always redispatch: a previous validation may have had a failed auto-merge follow-up.
    await dispatch(api, path, RELEASE_BRANCH, {
      release_pr: String(pr.number),
      release_sha: pr.head.sha,
    });
  }
  return { release_sha: '', version: '', release_pr_number: '' };
}

export async function publish({
  env,
  api,
  cwd = process.cwd(),
  parse = parseVersions,
  readAssets = localAssets,
  readVersions = localVersions,
  checkoutSha = (dir) =>
    execFileSync('git', ['rev-parse', 'HEAD'], { cwd: dir, encoding: 'utf8' }).trim(),
}) {
  const {
    GITHUB_REPOSITORY: repository,
    RELEASE_SHA: sha,
    RELEASE_VERSION: version,
    RELEASE_PR_NUMBER: number,
  } = env;
  const path = repoPath(repository);
  requireValue(env.GITHUB_REF === 'refs/heads/main', 'Release publication requires main');
  requireValue(
    SHA.test(sha) && VERSION.test(version) && /^[1-9]\d*$/.test(number),
    'Invalid publication inputs',
  );
  requireValue(checkoutSha(cwd) === sha, 'Checkout does not match release SHA');
  requireValue(parse(readVersions(cwd)) === version, 'Local release version mismatch');
  const assets = readAssets(cwd, version);
  validateAssets(assets, version);
  const pr = await api('GET', `${path}/pulls/${number}`);
  requireValue(String(pr.number) === number, 'Release PR number mismatch');
  const planned = await candidate(api, repository, pr, parse, env.RELEASE_APP_SLUG);
  requireValue(
    planned.release_sha === sha && planned.version === version,
    'Merged release metadata mismatch',
  );
  const tag = `v${version}`;
  const target = await tagSha(api, path, tag);
  requireValue(!target || target === sha, 'Release tag SHA mismatch');
  let release = await findRelease(api, path, tag);
  if (release && !release.draft) {
    requireValue(target === sha, 'Published release tag is missing');
    await verifyRemoteAssets(api, path, release, version);
  } else {
    if (!target) await api('POST', `${path}/git/refs`, { ref: `refs/tags/${tag}`, sha });
    if (!release)
      release = await api('POST', `${path}/releases`, {
        tag_name: tag,
        target_commitish: sha,
        name: tag,
        body: pr.body ?? '',
        draft: true,
        prerelease: false,
      });
    // Restart the entire draft asset set, including its checksum file. Never mix builds.
    for (const asset of await list(api, `${path}/releases/${release.id}/assets`)) {
      await api('DELETE', `${path}/releases/assets/${asset.id}`);
    }
    for (const name of assetNames(version)) {
      await api(
        'POST',
        `https://uploads.github.com/repos/${repository}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`,
        assets[name],
        { upload: true },
      );
    }
    await verifyRemoteAssets(api, path, release, version, assets);
    // Recheck the ref immediately before making the complete release public.
    requireValue((await tagSha(api, path, tag)) === sha, 'Release tag SHA mismatch');
    await api('PATCH', `${path}/releases/${release.id}`, { draft: false, make_latest: 'true' });
  }
  await markPublished(api, path, pr);
  await dispatch(api, path, 'main');
}

export function writeOutputs(path, outputs) {
  for (const [name, value] of Object.entries(outputs)) {
    const delimiter = randomUUID();
    appendFileSync(path, `${name}<<${delimiter}\n${value}\n${delimiter}\n`);
  }
}

async function main() {
  const env = process.env;
  const api = createGitHub({ token: env.GITHUB_TOKEN });
  const mode = process.argv[2];
  let outputs;
  if (mode === 'context')
    outputs = await context({
      env,
      event: JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, 'utf8')),
      api,
    });
  else if (mode === 'prepare') outputs = await prepare({ env, api });
  else if (mode === 'publish') await publish({ env, api });
  else throw new Error('Usage: node scripts/release.mjs context|prepare|publish');
  if (outputs && env.GITHUB_OUTPUT) writeOutputs(env.GITHUB_OUTPUT, outputs);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    // Library errors can embed request headers; do not log arbitrary error objects.
    console.error(
      error instanceof ReleaseError
        ? error.message
        : 'Release command failed; verify event metadata, release state, and API permissions.',
    );
    process.exitCode = 1;
  });
}
