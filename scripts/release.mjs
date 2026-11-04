import { appendFileSync } from 'node:fs';
import semanticRelease from 'semantic-release';
import { analysisPlugins } from '../release.config.js';

const mode = process.argv[2];
if (!['plan', 'publish'].includes(mode)) {
  throw new Error('Usage: node scripts/release.mjs plan|publish');
}
if (mode === 'publish' && !process.env.RELEASE_VERSION) {
  throw new Error('RELEASE_VERSION must identify the version built by the matrix');
}

const result = await semanticRelease(
  mode === 'plan' ? { dryRun: true, plugins: analysisPlugins } : {},
);

if (mode === 'plan') {
  const version = result?.nextRelease?.version ?? '';
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
  }
  console.log(version ? `Planned release: ${version}` : 'No release needed');
} else if (!result?.nextRelease) {
  throw new Error('Planned release was not published; main may have advanced');
}
