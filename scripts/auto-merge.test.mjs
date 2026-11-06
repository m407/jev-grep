import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { Script } from 'node:vm';

const workflow = readFileSync(
  new URL('../.github/workflows/dependabot-auto-merge.yml', import.meta.url),
  'utf8',
);
// Execute the actual last github-script block, with no YAML dependency or copied logic.
const blocks = [...workflow.matchAll(/^          script: \|\n((?:^            .*\n|^\n)+)/gm)];
assert.ok(blocks.length > 0, 'auto-merge workflow must contain a github-script block');
const source = blocks.at(-1)[1].replace(/^            /gm, '');
const script = new Script(`(async () => {\n${source}\n})()`, {
  filename: 'dependabot-auto-merge.yml:github-script',
});
const repo = { owner: 'm407', repo: 'jev-grep' };
const repository = { full_name: 'm407/jev-grep' };
const testedSha = 'a'.repeat(40);
const newSha = 'b'.repeat(40);
const releaseBranch = 'release-please--branches--main';
const releaseConfig = JSON.parse(
  readFileSync(new URL('../release-please-config.json', import.meta.url), 'utf8'),
);

function fixture(kind = 'release', event = 'workflow_dispatch') {
  const release = kind === 'release';
  const pr = {
    number: 42,
    title: release ? 'chore(main): release 1.2.0' : 'chore(deps): bump example from 1 to 2',
    state: 'open',
    draft: false,
    user: { login: release ? 'jev-release[bot]' : 'dependabot[bot]' },
    head: {
      repo: { ...repository },
      ref: release ? releaseBranch : 'dependabot/npm_and_yarn/example-2',
      sha: testedSha,
    },
    base: { repo: { ...repository }, ref: 'main' },
    labels: release ? [{ name: releaseConfig.label }] : [],
    changed_files: 4,
  };
  const state = {
    run: {
      id: 100,
      workflow_id: 200,
      status: 'completed',
      conclusion: 'success',
      event,
      head_sha: testedSha,
      head_branch: pr.head.ref,
      repository: { ...repository },
      head_repository: { ...repository },
    },
    workflow: { id: 200, path: '.github/workflows/release.yml' },
    jobs: ['check', 'commitlint / commitlint'].map((name) => ({
      name,
      run_id: 100,
      status: 'completed',
      conclusion: 'success',
    })),
    pr,
    latest: null,
    files: ['Cargo.toml', 'Cargo.lock', '.release-please-manifest.json', 'CHANGELOG.md'].map(
      (filename) => ({ filename, status: 'modified' }),
    ),
    candidates: [{ number: pr.number }],
    result: { merged: true, sha: 'c'.repeat(40) },
    mergeError: null,
    calls: [],
    messages: [],
    env: { RELEASE_APP_SLUG: 'jev-release' },
  };
  // Intentionally separate payload and API data: forged payload fields must not authorize merging.
  const context = { repo, payload: { workflow_run: structuredClone(state.run) } };
  let gets = 0;
  function endpoint(name, response) {
    return async (params) => {
      assert.equal(params.owner, repo.owner);
      assert.equal(params.repo, repo.repo);
      state.calls.push({ name, params: structuredClone(params) });
      return { data: structuredClone(await response(params)) };
    };
  }
  const github = {
    rest: {
      actions: {
        getWorkflowRun: endpoint('getRun', (params) => {
          assert.equal(params.run_id, 100);
          return state.run;
        }),
        getWorkflow: endpoint('getWorkflow', (params) => {
          assert.equal(params.workflow_id, 'release.yml');
          return state.workflow;
        }),
        listJobsForWorkflowRun: endpoint('jobs', (params) => {
          assert.equal(params.run_id, state.run.id);
          assert.equal(params.filter, 'latest');
          return state.jobs;
        }),
        createWorkflowDispatch: endpoint('dispatch', () => ({})),
      },
      repos: {
        listPullRequestsAssociatedWithCommit: endpoint('candidates', (params) => {
          assert.equal(params.commit_sha, state.run.head_sha);
          return state.candidates;
        }),
      },
      pulls: {
        get: endpoint('getPR', (params) => {
          assert.equal(params.pull_number, state.pr.number);
          gets += 1;
          return gets === 1 ? state.pr : state.latest || state.pr;
        }),
        listFiles: endpoint('files', (params) => {
          assert.equal(params.pull_number, state.pr.number);
          return state.files;
        }),
        merge: endpoint('merge', () => {
          if (state.mergeError) throw state.mergeError;
          return state.result;
        }),
      },
    },
    paginate: async (method, params) => (await method(params)).data,
  };
  const core = { info: (message) => state.messages.push(message) };
  state.execute = () =>
    script.runInNewContext({ github, context, core, process: { env: state.env } });
  state.context = context;
  return state;
}

function mutations(state) {
  return state.calls.filter(({ name }) => name === 'merge' || name === 'dispatch');
}

test('accepts the exact pending label from the actual release-please configuration', async () => {
  assert.equal(releaseConfig.label, 'autorelease: pending');
  assert.equal(releaseConfig['release-label'], 'autorelease: tagged');
  const state = fixture();
  await state.execute();
  assert.deepEqual(
    mutations(state).map(({ name }) => name),
    ['merge', 'dispatch'],
  );
});

for (const [kind, event] of [
  ['dependabot', 'pull_request'],
  ['release', 'pull_request'],
  ['release', 'workflow_dispatch'],
]) {
  test(`merges tested ${kind} head after ${event}, then explicitly dispatches main`, async () => {
    const state = fixture(kind, event);
    await state.execute();
    assert.deepEqual(mutations(state), [
      {
        name: 'merge',
        params: {
          ...repo,
          pull_number: 42,
          sha: testedSha,
          merge_method: 'squash',
          commit_title: `${state.pr.title} (#42)`,
        },
      },
      {
        name: 'dispatch',
        params: { ...repo, workflow_id: 'release.yml', ref: 'main', inputs: {} },
      },
    ]);
    const mergeIndex = state.calls.findIndex(({ name }) => name === 'merge');
    assert.equal(state.calls[mergeIndex - 1].name, 'getPR');
    assert.equal(state.calls.filter(({ name }) => name === 'getPR').length, 2);
    assert.equal(
      state.calls.some(({ name }) => name === 'files'),
      kind === 'release',
    );
  });
}

const invalidRuns = [
  ['a different workflow ID despite a trusted payload', (s) => (s.run.workflow_id = 999)],
  ['a different workflow path', (s) => (s.workflow.path = '.github/workflows/other.yml')],
  ['a failed run despite a successful payload', (s) => (s.run.conclusion = 'failure')],
  ['an unfinished run', (s) => (s.run.status = 'in_progress')],
  ['a fork run', (s) => (s.run.head_repository.full_name = 'other/jev-grep')],
  ['a different run repository', (s) => (s.run.repository.full_name = 'other/jev-grep')],
  ['a push run', (s) => (s.run.event = 'push')],
  ['a dispatch on main', (s) => (s.run.head_branch = 'main')],
  ['a dispatch on an arbitrary branch', (s) => (s.run.head_branch = 'chore/dev-quality-gates')],
  ['a skipped check job', (s) => (s.jobs[0].conclusion = 'skipped')],
  ['a failed check job', (s) => (s.jobs[0].conclusion = 'failure')],
  ['a skipped commitlint job', (s) => (s.jobs[1].conclusion = 'skipped')],
  ['a failed commitlint job', (s) => (s.jobs[1].conclusion = 'failure')],
  ['an unfinished job', (s) => (s.jobs[0].status = 'in_progress')],
  ['a missing required job', (s) => s.jobs.pop()],
  ['a mismatched job name', (s) => (s.jobs[1].name = 'commitlint')],
  ['a job from another run', (s) => (s.jobs[0].run_id = 999)],
  ['ambiguous required jobs', (s) => s.jobs.push({ ...s.jobs[0] })],
];
for (const [name, change] of invalidRuns) {
  test(`rejects ${name}`, async () => {
    const state = fixture();
    change(state);
    await state.execute();
    assert.deepEqual(mutations(state), []);
  });
}

const invalidPRs = [
  ['the legacy Actions bot', (pr) => (pr.user.login = 'github-actions[bot]')],
  ['a stale tested SHA', (pr) => (pr.head.sha = newSha)],
  ['a human author', (pr) => (pr.user.login = 'maintainer')],
  ['another bot author', (pr) => (pr.user.login = 'release-please[bot]')],
  ['a fork source', (pr) => (pr.head.repo.full_name = 'other/jev-grep')],
  ['a missing source repository', (pr) => (pr.head.repo = null)],
  ['a foreign base repository', (pr) => (pr.base.repo.full_name = 'other/jev-grep')],
  ['a non-main base', (pr) => (pr.base.ref = 'develop')],
  ['a closed PR', (pr) => (pr.state = 'closed')],
  ['a draft release', (pr) => (pr.draft = true)],
  ['the github-actions migration PR', (pr) => (pr.head.ref = 'chore/dev-quality-gates')],
  ['a similarly named branch', (pr) => (pr.head.ref = `${releaseBranch}-extra`)],
  ['missing pending label', (pr) => (pr.labels = [])],
  ['an unspaced pending label', (pr) => (pr.labels = [{ name: 'autorelease:pending' }])],
  ['the configured tagged label', (pr) => pr.labels.push({ name: releaseConfig['release-label'] })],
  ['the spaced snooze label', (pr) => pr.labels.push({ name: 'autorelease: snooze' })],
];
for (const [name, change] of invalidPRs) {
  for (const race of [false, true]) {
    test(`rejects ${name}${race ? ' immediately before merging' : ''}`, async () => {
      const state = fixture();
      if (race) {
        state.latest = structuredClone(state.pr);
        change(state.latest);
      } else {
        change(state.pr);
      }
      await state.execute();
      assert.deepEqual(mutations(state), []);
    });
  }
}

for (const filename of ['.github/workflows/release.yml', 'src/main.rs', 'package.json']) {
  test(`release file guard rejects ${filename}`, async () => {
    const state = fixture();
    state.files[0].filename = filename;
    await state.execute();
    assert.deepEqual(mutations(state), []);
  });
}

test('release file guard rejects a forbidden path renamed to an allowed path', async () => {
  const state = fixture();
  state.files[0].previous_filename = '.github/workflows/release.yml';
  state.files[0].status = 'renamed';
  await state.execute();
  assert.deepEqual(mutations(state), []);
});

for (const [name, change] of [
  ['incomplete file listings', (s) => (s.pr.changed_files = 3001)],
  [
    'empty file listings',
    (s) => {
      s.files = [];
      s.pr.changed_files = 0;
    },
  ],
  [
    'a changed file count before merging',
    (s) => {
      s.latest = structuredClone(s.pr);
      s.latest.changed_files += 1;
    },
  ],
]) {
  test(`release file guard rejects ${name}`, async () => {
    const state = fixture();
    change(state);
    await state.execute();
    assert.deepEqual(mutations(state), []);
  });
}

test('Dependabot major updates can change files outside the release-only allowlist', async () => {
  const state = fixture('dependabot', 'pull_request');
  state.files = [{ filename: '.github/workflows/release.yml' }];
  await state.execute();
  assert.equal(mutations(state).length, 2);
});

test('dispatch cannot authorize Dependabot even on the release branch', async () => {
  const state = fixture('dependabot');
  state.run.head_branch = releaseBranch;
  state.pr.head.ref = releaseBranch;
  await state.execute();
  assert.deepEqual(mutations(state), []);
});

test('missing or invalid App configuration cannot authorize release merging', async () => {
  for (const slug of [
    undefined,
    '',
    'github-actions',
    'dependabot',
    'jev-release[bot]',
    'other-app',
  ]) {
    const state = fixture();
    state.env.RELEASE_APP_SLUG = slug;
    await state.execute();
    assert.deepEqual(mutations(state), []);
  }
});

test('Dependabot merging does not require release App credentials', async () => {
  const state = fixture('dependabot', 'pull_request');
  state.env = {};
  await state.execute();
  assert.equal(mutations(state).length, 2);
});

test('PR run branch must match the current PR head branch', async () => {
  const state = fixture('dependabot', 'pull_request');
  state.run.head_branch = 'other';
  await state.execute();
  assert.deepEqual(mutations(state), []);
});

test('untrusted payload SHA, branch, author and PR metadata cannot select a merge', async () => {
  const state = fixture();
  Object.assign(state.context.payload.workflow_run, {
    head_sha: newSha,
    head_branch: 'untrusted',
    pull_requests: [{ number: 999, user: { login: 'github-actions[bot]' } }],
  });
  state.candidates = [];
  await state.execute();
  assert.deepEqual(mutations(state), []);
  assert.equal(state.calls.find(({ name }) => name === 'candidates').params.commit_sha, testedSha);
});

for (const throws of [false, true]) {
  test(`merge ${throws ? 'API exception' : 'refusal'} blocks dispatch`, async () => {
    const state = fixture();
    if (throws) state.mergeError = new Error('Head changed during merge');
    else state.result = { merged: false, message: 'Merge conflict' };
    await assert.rejects(state.execute(), /Head changed during merge|Merge conflict/);
    assert.deepEqual(
      mutations(state).map(({ name }) => name),
      ['merge'],
    );
  });
}
