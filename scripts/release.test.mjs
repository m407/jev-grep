import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import {
  RELEASE_BRANCH,
  assetNames,
  context,
  createGitHub,
  parseVersions,
  prepare,
  publish,
  validateAssets,
} from './release.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const repository = 'example/jev-grep';
const sha = 'a'.repeat(40);
const head = 'b'.repeat(40);
const base = 'c'.repeat(40);
const pending = 'autorelease: pending';
const tagged = 'autorelease: tagged';
const version = '1.2.3';
const baseEnv = {
  GITHUB_REPOSITORY: repository,
  GITHUB_REF: 'refs/heads/main',
  GITHUB_SHA: sha,
  GITHUB_TOKEN: 'fixture-token',
  RELEASE_APP_SLUG: 'jev-release',
  RELEASE_APP_TOKEN_SLUG: 'jev-release',
  RELEASE_PR_TOKEN: 'fixture-app-token',
};

function pull(overrides = {}) {
  return {
    number: 12,
    title: 'chore(main): release 1.2.3',
    user: { login: 'jev-release[bot]' },
    base: { ref: 'main', sha: base, repo: { full_name: repository } },
    head: { ref: RELEASE_BRANCH, sha: head, repo: { full_name: repository } },
    labels: [{ name: pending }],
    draft: false,
    state: 'open',
    merged_at: null,
    body: 'Release notes',
    ...overrides,
  };
}

function files(releaseVersion = version) {
  return {
    'Cargo.toml': `[package]\nname = "jev-grep"\nversion = "${releaseVersion}"\n`,
    'Cargo.lock': `version = 4\n[[package]]\nname = "jev-grep"\nversion = "${releaseVersion}"\n\n[[package]]\nname = "dependency"\nversion = "9.8.7"\nsource = "registry+https://example.invalid"\n`,
    '.release-please-manifest.json': JSON.stringify({ '.': releaseVersion }),
  };
}

function assets() {
  const result = Object.fromEntries(
    assetNames(version)
      .slice(0, 3)
      .map((name) => [name, Buffer.from(`archive ${name}`)]),
  );
  result.SHA256SUMS = Buffer.from(
    Object.entries(result)
      .map(([name, bytes]) => `${createHash('sha256').update(bytes).digest('hex')}  ${name}\n`)
      .join(''),
  );
  return result;
}

// Stateful GitHub boundary: retries see real mutations made by the previous call.
function githubFixture({ merged = true } = {}) {
  const state = {
    pr: pull(
      merged ? { state: 'closed', merged_at: '2026-11-01T00:00:00Z', merge_commit_sha: sha } : {},
    ),
    files: files(),
    changed: ['Cargo.toml', 'Cargo.lock', '.release-please-manifest.json', 'CHANGELOG.md'],
    ancestor: true,
    tag: null,
    release: null,
    assets: [],
    calls: [],
    dispatches: [],
    uploadCount: 0,
    failUpload: 0,
    nextId: 1,
  };
  const api = async (method, path, body, options = {}) => {
    state.calls.push({ method, path, body, options });
    const url = new URL(path, 'https://api.github.com');
    const route = url.pathname.replace(`/repos/${repository}`, '');
    if (method === 'GET' && route === '/issues')
      return state.pr.merged_at && state.pr.labels.some((label) => label.name === pending)
        ? [{ number: 12, pull_request: {} }]
        : [];
    if (method === 'GET' && route === '/pulls')
      return state.pr.state === 'open' ? [{ number: 12 }] : [];
    if (method === 'GET' && route === '/pulls/12') return structuredClone(state.pr);
    if (method === 'GET' && route === '/pulls/12/files')
      return state.changed.map((filename) => ({ filename }));
    if (method === 'GET' && route === `/compare/${sha}...main`)
      return {
        status: state.ancestor ? 'ahead' : 'diverged',
        merge_base_commit: { sha: state.ancestor ? sha : base },
      };
    if (method === 'GET' && route.startsWith('/contents/')) {
      assert.equal(url.searchParams.get('ref'), sha, 'metadata must come from immutable merge SHA');
      return {
        type: 'file',
        encoding: 'base64',
        content: Buffer.from(state.files[route.slice('/contents/'.length)]).toString('base64'),
      };
    }
    if (method === 'GET' && route === `/git/ref/tags/v${version}`)
      return state.tag ? { object: { type: 'commit', sha: state.tag } } : null;
    if (method === 'POST' && route === '/git/refs') {
      assert.equal(body.ref, `refs/tags/v${version}`);
      state.tag = body.sha;
      return {};
    }
    if (method === 'GET' && route === '/releases')
      return state.release ? [structuredClone(state.release)] : [];
    if (method === 'POST' && route === '/releases') {
      assert.equal(body.draft, true, 'release must start as draft');
      assert.equal(body.target_commitish, sha);
      state.release = { id: 1, ...body };
      return structuredClone(state.release);
    }
    if (method === 'GET' && route === '/releases/1/assets')
      return state.assets.map(({ bytes: _bytes, ...metadata }) => metadata);
    if (method === 'GET' && route.startsWith('/releases/assets/')) {
      assert.equal(options.binary, true);
      return state.assets.find((asset) => asset.id === Number(route.split('/').at(-1))).bytes;
    }
    if (method === 'DELETE' && route.startsWith('/releases/assets/')) {
      state.assets = state.assets.filter((asset) => asset.id !== Number(route.split('/').at(-1)));
      return null;
    }
    if (method === 'POST' && url.host === 'uploads.github.com') {
      assert.equal(state.release.draft, true);
      state.uploadCount++;
      if (state.uploadCount === state.failUpload) throw new Error('Interrupted upload');
      state.assets.push({
        id: state.nextId++,
        name: url.searchParams.get('name'),
        size: body.length,
        bytes: body,
        state: 'uploaded',
      });
      return {};
    }
    if (method === 'PATCH' && route === '/releases/1') {
      validateAssets(
        Object.fromEntries(state.assets.map((asset) => [asset.name, asset.bytes])),
        version,
      );
      assert.equal(state.tag, sha);
      Object.assign(state.release, body);
      return {};
    }
    if (method === 'POST' && route === '/issues/12/labels') {
      for (const name of body.labels)
        if (!state.pr.labels.some((label) => label.name === name)) state.pr.labels.push({ name });
      return {};
    }
    if (method === 'DELETE' && route === `/issues/12/labels/${encodeURIComponent(pending)}`) {
      state.pr.labels = state.pr.labels.filter((label) => label.name !== pending);
      return null;
    }
    if (method === 'POST' && route === '/actions/workflows/release.yml/dispatches') {
      state.dispatches.push(body);
      return null;
    }
    throw new Error(`Unexpected fixture call ${method} ${path}`);
  };
  return { state, api };
}

function publishing(fixture, overrides = {}) {
  return publish({
    env: { ...baseEnv, RELEASE_SHA: sha, RELEASE_VERSION: version, RELEASE_PR_NUMBER: '12' },
    api: fixture.api,
    checkoutSha: () => sha,
    readVersions: () => files(),
    readAssets: assets,
    ...overrides,
  });
}

test('commitlint accepts long body lines while enforcing Conventional Commit headers', () => {
  const lint = (message) =>
    spawnSync(process.execPath, [join(root, 'node_modules/@commitlint/cli/cli.js')], {
      cwd: root,
      input: message,
      encoding: 'utf8',
    });
  const valid = lint(`docs: explain CLI usage\n\n${'Detailed documentation. '.repeat(10)}\n`);
  assert.equal(valid.status, 0, valid.stdout + valid.stderr);
  for (const message of ['Add documentation\n', 'unknown: add documentation\n']) {
    const invalid = lint(message);
    assert.equal(invalid.status, 1, invalid.stdout + invalid.stderr);
    assert.match(invalid.stdout, /type-(empty|enum)/);
  }
});

test('normal PR checks merge snapshot and exposes actual head/base for commitlint', async () => {
  const pr = pull({
    user: { login: 'contributor' },
    head: { ...pull().head, repo: { full_name: 'fork/jev-grep' } },
  });
  const result = await context({
    env: { ...baseEnv, GITHUB_EVENT_NAME: 'pull_request', GITHUB_REF: 'refs/pull/12/merge' },
    event: { repository: { full_name: repository }, number: 12, pull_request: pr },
    api: () => assert.fail('PR context must not write or fetch'),
  });
  assert.deepEqual(result, {
    check_sha: sha,
    pr_number: '12',
    pr_title: pr.title,
    base_sha: base,
    head_sha: head,
  });
});

async function dispatchContext(pr = pull(), input = {}, env = {}) {
  return context({
    env: {
      ...baseEnv,
      GITHUB_EVENT_NAME: 'workflow_dispatch',
      GITHUB_REF: `refs/heads/${RELEASE_BRANCH}`,
      GITHUB_SHA: head,
      ...env,
    },
    event: {
      repository: { full_name: repository },
      inputs: { release_pr: '12', release_sha: head, ...input },
    },
    api: async (method, path) => {
      assert.equal(method, 'GET');
      assert.equal(path, `/repos/${repository}/pulls/12`);
      return pr;
    },
  });
}

test('release dispatch checks out exact head', async () => {
  assert.equal((await dispatchContext()).check_sha, head);
});

for (const [name, alter] of [
  [
    'wrong bot',
    (pr) => {
      pr.user.login = 'dependabot[bot]';
    },
  ],
  [
    'wrong branch',
    (pr) => {
      pr.head.ref = 'other';
    },
  ],
  [
    'fork',
    (pr) => {
      pr.head.repo.full_name = 'fork/jev-grep';
    },
  ],
  [
    'wrong base',
    (pr) => {
      pr.base.ref = 'other';
    },
  ],
  [
    'missing pending label',
    (pr) => {
      pr.labels = [];
    },
  ],
  [
    'already tagged',
    (pr) => {
      pr.labels.push({ name: tagged });
    },
  ],
  [
    'snoozed',
    (pr) => {
      pr.labels.push({ name: 'autorelease: snooze' });
    },
  ],
  [
    'closed',
    (pr) => {
      pr.state = 'closed';
    },
  ],
  [
    'draft',
    (pr) => {
      pr.draft = true;
    },
  ],
  [
    'stale head',
    (pr) => {
      pr.head.sha = sha;
    },
  ],
]) {
  test(`context rejects ${name}`, async () => {
    const pr = pull();
    alter(pr);
    await assert.rejects(dispatchContext(pr));
  });
}

test('context rejects dispatch ref/input combinations and changed run SHA', async () => {
  await assert.rejects(dispatchContext(pull(), {}, { GITHUB_REF: 'refs/heads/main' }), /branch/);
  await assert.rejects(dispatchContext(pull(), { release_sha: '' }), /inputs/);
  await assert.rejects(dispatchContext(pull(), {}, { GITHUB_SHA: sha }), /Stale/);
  await assert.rejects(dispatchContext(pull(), { release_pr: '', release_sha: '' }), /main/);
  for (const eventName of ['push', 'workflow_dispatch']) {
    const result = await context({
      env: { ...baseEnv, GITHUB_EVENT_NAME: eventName },
      event: { repository: { full_name: repository }, inputs: {} },
    });
    assert.equal(result.check_sha, sha);
    assert.equal(result.pr_number, '');
  }
});

test('verify_builds is read-only on any branch and validates checkout versions', async () => {
  const options = {
    env: {
      ...baseEnv,
      GITHUB_EVENT_NAME: 'workflow_dispatch',
      GITHUB_REF: 'refs/heads/chore/dev-quality-gates',
    },
    event: { repository: { full_name: repository }, inputs: { verify_builds: true } },
    api: () => assert.fail('Build verification must not call GitHub'),
    readVersions: () => files(),
  };
  for (const enabled of [true, 'true']) {
    const result = await context({
      ...options,
      event: { ...options.event, inputs: { verify_builds: enabled } },
    });
    assert.deepEqual(result, {
      check_sha: sha,
      pr_number: '',
      pr_title: '',
      base_sha: '',
      head_sha: '',
      version,
    });
  }
  for (const name of ['release_pr', 'release_sha']) {
    await assert.rejects(
      context({
        ...options,
        event: {
          ...options.event,
          inputs: { verify_builds: true, [name]: name === 'release_pr' ? '12' : head },
        },
      }),
      /cannot include/,
    );
  }
  const inconsistent = files();
  inconsistent['.release-please-manifest.json'] = '{".":"9.0.0"}';
  await assert.rejects(context({ ...options, readVersions: () => inconsistent }));
  await assert.rejects(
    context({ ...options, env: { ...options.env, GITHUB_REF: 'refs/tags/v1.1.0' } }),
    /requires a branch/,
  );
  for (const disabled of [false, 'false']) {
    const event = { ...options.event, inputs: { verify_builds: disabled } };
    await assert.rejects(context({ ...options, event }), /requires main/);
    assert.equal(
      (await context({ ...options, env: { ...options.env, GITHUB_REF: 'refs/heads/main' }, event }))
        .version,
      undefined,
    );
  }
});

test('metadata parser validates all three versions and ignores dependency versions', () => {
  assert.equal(parseVersions(files()), version);
  for (const name of ['Cargo.toml', 'Cargo.lock', '.release-please-manifest.json']) {
    const mismatched = files();
    mismatched[name] = mismatched[name].replace(version, '2.0.0');
    assert.throws(() => parseVersions(mismatched));
  }
  assert.equal(
    parseVersions({
      'Cargo.toml': readFileSync(join(root, 'Cargo.toml'), 'utf8'),
      'Cargo.lock': readFileSync(join(root, 'Cargo.lock'), 'utf8'),
      '.release-please-manifest.json': readFileSync(
        join(root, '.release-please-manifest.json'),
        'utf8',
      ),
    }),
    JSON.parse(readFileSync(join(root, '.release-please-manifest.json'), 'utf8'))['.'],
  );
});

test('prepare selects immutable merged release before invoking release-please', async () => {
  const fixture = githubFixture();
  assert.deepEqual(
    await prepare({
      env: baseEnv,
      api: fixture.api,
      createPRs: () => assert.fail('publication first'),
    }),
    { release_sha: sha, version, release_pr_number: '12' },
  );
  assert.equal(
    fixture.state.calls.some((call) => call.method !== 'GET'),
    false,
  );
});

test('prepare rejects version drift, non-ancestor merge and unexpected release files', async () => {
  for (const alter of [
    (state) => {
      state.files['.release-please-manifest.json'] = '{".":"9.0.0"}';
    },
    (state) => {
      state.ancestor = false;
    },
    (state) => {
      state.changed.push('src/main.rs');
    },
    (state) => {
      state.tag = head;
    },
  ]) {
    const fixture = githubFixture();
    alter(fixture.state);
    await assert.rejects(
      prepare({
        env: baseEnv,
        api: fixture.api,
        createPRs: () => assert.fail('invalid candidate'),
      }),
    );
    assert.equal(
      fixture.state.calls.some((call) => call.method !== 'GET'),
      false,
    );
  }
});

test('unchanged release-please result still dispatches discovered PR validation', async () => {
  const fixture = githubFixture({ merged: false });
  let calls = 0;
  const result = await prepare({
    env: baseEnv,
    api: fixture.api,
    createPRs: async (options) => {
      assert.deepEqual(options, { repository, token: baseEnv.RELEASE_PR_TOKEN });
      calls++;
      return [];
    },
  });
  assert.equal(calls, 1);
  assert.equal(result.release_sha, '');
  assert.deepEqual(fixture.state.dispatches, [
    { ref: RELEASE_BRANCH, inputs: { release_pr: '12', release_sha: head } },
  ]);
});

test('prepare retry redispatches unchanged PR so failed auto-merge can recover', async () => {
  const fixture = githubFixture({ merged: false });
  await prepare({ env: baseEnv, api: fixture.api, createPRs: async () => [] });
  await prepare({ env: baseEnv, api: fixture.api, createPRs: async () => [] });
  assert.deepEqual(fixture.state.dispatches, [
    { ref: RELEASE_BRANCH, inputs: { release_pr: '12', release_sha: head } },
    { ref: RELEASE_BRANCH, inputs: { release_pr: '12', release_sha: head } },
  ]);
});

test('PR creation fails closed for missing credentials or mismatched App identity', async () => {
  for (const override of [
    { RELEASE_PR_TOKEN: '' },
    { RELEASE_APP_SLUG: '' },
    { RELEASE_APP_SLUG: 'github-actions', RELEASE_APP_TOKEN_SLUG: 'github-actions' },
    { RELEASE_APP_TOKEN_SLUG: 'another-app' },
  ]) {
    const fixture = githubFixture({ merged: false });
    await assert.rejects(
      prepare({
        env: { ...baseEnv, ...override },
        api: fixture.api,
        createPRs: () => assert.fail('must not create PRs'),
      }),
    );
    assert.deepEqual(fixture.state.dispatches, []);
  }
});

test('legacy and unrelated bot PRs cannot authorize validation or publication', async () => {
  for (const login of ['github-actions[bot]', 'other-app[bot]']) {
    await assert.rejects(dispatchContext(pull({ user: { login } })), /author/);
    const fixture = githubFixture();
    fixture.state.pr.user.login = login;
    await assert.rejects(prepare({ env: baseEnv, api: fixture.api }), /author/);
    assert.equal(
      fixture.state.calls.some((call) => call.method !== 'GET'),
      false,
    );
  }
});

test('publish rejects missing or invalid assets before any API calls', async () => {
  for (const alter of [
    (set) => {
      delete set[assetNames(version)[0]];
    },
    (set) => {
      set.extra = Buffer.from('unexpected');
    },
    (set) => {
      set[assetNames(version)[0]] = Buffer.from('corrupt');
    },
    (set) => {
      set.SHA256SUMS = Buffer.from('invalid');
    },
  ]) {
    const fixture = githubFixture();
    const set = assets();
    alter(set);
    await assert.rejects(publishing(fixture, { readAssets: () => set }));
    assert.equal(fixture.state.calls.length, 0);
  }
});

test('publish rejects wrong checkout, version and tag without mutating GitHub', async () => {
  for (const overrides of [
    { checkoutSha: () => head },
    { readVersions: () => files('9.0.0') },
    {},
  ]) {
    const fixture = githubFixture();
    fixture.state.tag = head;
    await assert.rejects(publishing(fixture, overrides));
    assert.equal(
      fixture.state.calls.some((call) => call.method !== 'GET'),
      false,
    );
  }
});

test('interrupted draft upload is recoverable and replaces the complete asset set', async () => {
  const fixture = githubFixture();
  fixture.state.failUpload = 3;
  await assert.rejects(publishing(fixture), /Interrupted upload/);
  assert.equal(fixture.state.release.draft, true);
  assert.equal(fixture.state.assets.length, 2);
  assert.deepEqual(fixture.state.pr.labels, [{ name: pending }]);
  fixture.state.failUpload = 0;
  await publishing(fixture);
  assert.equal(fixture.state.release.draft, false);
  assert.equal(fixture.state.assets.length, 4);
  assert.deepEqual(fixture.state.pr.labels, [{ name: tagged }]);
  assert.equal(
    fixture.state.calls.filter(
      (call) => call.method === 'DELETE' && call.path.includes('/releases/assets/'),
    ).length,
    2,
  );
  assert.deepEqual(fixture.state.dispatches, [{ ref: 'main', inputs: {} }]);
});

test('published retry verifies remote checksums and leaves tag/release/assets untouched', async () => {
  const fixture = githubFixture();
  await publishing(fixture);
  fixture.state.calls = [];
  await publishing(fixture);
  assert.equal(
    fixture.state.calls.some(
      (call) =>
        call.method !== 'GET' && (call.path.includes('/releases') || call.path.includes('/git/')),
    ),
    false,
  );
  assert.equal(fixture.state.dispatches.length, 2);
  fixture.state.assets[0].bytes = Buffer.from('corruption');
  await assert.rejects(publishing(fixture), /size mismatch|Checksum mismatch/);
});

test('prepare repairs interrupted pending-to-tagged transition before generating next PR', async () => {
  const fixture = githubFixture();
  await publishing(fixture);
  fixture.state.pr.labels.push({ name: pending });
  let generated = false;
  const result = await prepare({
    env: baseEnv,
    api: fixture.api,
    createPRs: async () => {
      generated = true;
    },
  });
  assert.equal(generated, true);
  assert.equal(result.version, '');
  assert.deepEqual(fixture.state.pr.labels, [{ name: tagged }]);
});

test('API transport authenticates JSON/uploads/downloads and never exposes response secrets', async () => {
  const requests = [];
  const api = createGitHub({
    token: 'fixture-token',
    fetchImpl: async (url, options) => {
      requests.push({ url, ...options });
      return new Response('{}', { status: 200 });
    },
  });
  await api('POST', '/repos/example/jev-grep/git/refs', { sha });
  await api(
    'POST',
    'https://uploads.github.com/repos/example/jev-grep/releases/1/assets?name=test',
    Buffer.from('bytes'),
    { upload: true },
  );
  await api('GET', '/repos/example/jev-grep/releases/assets/1', undefined, { binary: true });
  assert.equal(requests[0].headers.Authorization, 'Bearer fixture-token');
  assert.equal(requests[0].body, JSON.stringify({ sha }));
  assert.equal(requests[1].headers['Content-Type'], 'application/octet-stream');
  assert.equal(requests[2].headers.Accept, 'application/octet-stream');
  const failing = createGitHub({
    token: 'fixture-token',
    fetchImpl: async () => new Response('secret response', { status: 403 }),
  });
  await assert.rejects(
    failing('GET', '/repos/example/jev-grep'),
    (error) => error.message === 'GitHub GET failed (403)',
  );
  await assert.rejects(api('GET', 'https://untrusted.invalid/'), /origin/);
});

test('CLI retains safe validation errors and redacts unknown errors', () => {
  const run = (mode, env) =>
    spawnSync(process.execPath, [join(root, 'scripts/release.mjs'), mode], {
      env: { ...process.env, ...baseEnv, ...env },
      encoding: 'utf8',
    });
  const known = run('publish', { GITHUB_REF: 'refs/heads/other' });
  assert.equal(known.status, 1);
  assert.equal(known.stderr.trim(), 'Release publication requires main');
  const unknown = run('context', { GITHUB_EVENT_PATH: '/missing/secret-marker' });
  assert.equal(unknown.status, 1);
  assert.match(unknown.stderr, /Release command failed/);
  assert.doesNotMatch(unknown.stderr, /secret-marker|fixture-token/);
});

async function libraryPlan(messages, previous = '1.1.0') {
  const { Manifest, VERSION: libraryVersion } = await import('release-please');
  assert.equal(libraryVersion, '17.11.2');
  const quiet = { debug() {}, info() {}, warn() {}, error() {} };
  const source = files(previous);
  const github = {
    repository: { owner: 'example', repo: 'jev-grep' },
    getFileJson: async (path, branch) => {
      assert.equal(branch, 'main');
      return path === 'release-please-config.json'
        ? JSON.parse(readFileSync(join(root, path), 'utf8'))
        : JSON.parse(source[path]);
    },
    getFileContentsOnBranch: async (path, branch) => {
      assert.equal(branch, 'main');
      assert.equal(path, 'Cargo.toml');
      return { parsedContent: source[path] };
    },
    async *releaseIterator() {
      yield { tagName: `v${previous}`, sha: base, notes: '' };
    },
    async *mergeCommitIterator() {
      for (const [index, message] of messages.entries()) {
        yield { sha: String(index + 1).padStart(40, '0'), message, files: ['src/main.rs'] };
      }
      yield { sha: base, message: `chore: release ${previous}`, files: ['Cargo.toml'] };
    },
  };
  const manifest = await Manifest.fromManifest(github, 'main', undefined, undefined, {
    logger: quiet,
  });
  return manifest.buildPullRequests();
}

for (const [messages, expected, previous = '1.1.0'] of [
  [['fix: handle timeout'], '1.1.1'],
  [['perf: reuse connections'], '1.1.1'],
  [['feat(cli): add output format'], '1.2.0'],
  [['feat(api)!: change response format'], '2.0.0'],
  [['fix: change response\n\nBREAKING CHANGE: remove old fields'], '2.0.0'],
  [['docs: explain installation'], null],
  [['chore: release main'], null],
  [['fix: timeout', 'feat: support JSON', 'docs: explain flags'], '1.2.0'],
  [['feat!: change API'], '1.0.0', '0.1.0'],
]) {
  test(`release-please classification: ${messages.join('; ').split('\n')[0]}`, async () => {
    const planned = await libraryPlan(messages, previous);
    assert.equal(planned.length, expected ? 1 : 0);
    if (expected) {
      assert.equal(planned[0].headRefName, RELEASE_BRANCH);
      assert.equal(planned[0].title.toString(), 'chore: release main');
      assert.equal(planned[0].body.releaseData[0].version.toString(), expected);
    }
  });
}

test('release-please updates Cargo and release manifest without changing dependencies', async () => {
  const source = Object.fromEntries(
    ['Cargo.toml', 'Cargo.lock', '.release-please-manifest.json'].map((name) => [
      name,
      readFileSync(join(root, name), 'utf8'),
    ]),
  );
  const previous = parseVersions(source);
  const [major, minor, patch] = previous.split('.');
  const next = `${major}.${minor}.${Number(patch) + 1}`;
  const [plan] = await libraryPlan(['fix: handle timeout'], previous);
  assert.deepEqual(plan.updates.map((update) => update.path).sort(), [
    '.release-please-manifest.json',
    'CHANGELOG.md',
    'Cargo.lock',
    'Cargo.toml',
  ]);
  const updated = {};
  const quiet = { debug() {}, info() {}, warn() {}, error() {} };
  for (const update of plan.updates.filter((update) => update.path !== 'CHANGELOG.md')) {
    updated[update.path] = update.updater.updateContent(source[update.path], quiet);
  }
  assert.equal(parseVersions(updated), next);
  for (const name of ['Cargo.toml', 'Cargo.lock']) {
    assert.equal(
      updated[name],
      source[name].replace(
        `name = "jev-grep"\nversion = "${previous}"`,
        `name = "jev-grep"\nversion = "${next}"`,
      ),
    );
  }
});
