import 'zx/globals';
import nodeAssert from 'node:assert';
import { REPO_ROOT } from '../meta/constants.js';

async function getLastVersion() {
  const pkgPath = path.resolve(REPO_ROOT, './packages/rolldown/package.json');
  const result = await import(pkgPath, {
    assert: {
      type: 'json',
    },
  });
  return result.default.version;
}

const gitUserName = await $`git config --global user.name`;
const gitUserEmail = await $`git config --global user.email`;

nodeAssert.strictEqual(gitUserName, 'github-actions[bot]');
nodeAssert.strictEqual(
  gitUserEmail,
  'github-actions[bot]@users.noreply.github.com',
);

const lastVersion = await getLastVersion();

await $`git tag v${lastVersion} -m "v${lastVersion}"`;
await $`git push origin --follow-tags`;
