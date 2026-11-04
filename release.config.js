export const analysisPlugins = [
  ['@semantic-release/commit-analyzer', { preset: 'conventionalcommits' }],
  ['@semantic-release/release-notes-generator', { preset: 'conventionalcommits' }],
];

export default {
  branches: ['main'],
  tagFormat: 'v${version}',
  plugins: [
    ...analysisPlugins,
    './scripts/release-prepare.mjs',
    [
      '@semantic-release/git',
      {
        assets: ['Cargo.toml', 'Cargo.lock'],
        message: 'chore(release): ${nextRelease.version} [skip ci]',
      },
    ],
    [
      '@semantic-release/github',
      {
        assets: ['dist/*.tar.gz', 'dist/*.zip', 'dist/SHA256SUMS'],
        successComment: false,
        failComment: false,
        failTitle: false,
        releasedLabels: false,
      },
    ],
  ],
};
