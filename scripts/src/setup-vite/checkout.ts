// Shared handling of the vendored Vite submodule (`vite/` at the repo root) —
// the single Vite checkout used by both the dev-server test harness
// (`packages/test-dev-server`, see `main.ts`) and Vite's own test suite
// (`packages/vite-tests/run.ts`, which clones this checkout locally).

import { execSync } from 'node:child_process';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';

// scripts/src/setup-vite/checkout.ts → repo root is three levels up.
export const repoRoot = nodePath.resolve(
  nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url)),
  '../../..',
);
export const viteDir = nodePath.join(repoRoot, 'vite');

export const run = (cmd: string, cwd: string): void => {
  console.log(`[setup-vite] ${cmd}`);
  execSync(cmd, { cwd, stdio: 'inherit' });
};
const capture = (cmd: string, cwd: string): string =>
  execSync(cmd, { cwd, encoding: 'utf8' }).trim();

// Ensure the submodule has a checkout, and use that checkout exactly as-is.
//
//   - Uninitialized (fresh clone / CI): `git submodule update --init` checks
//     out the pinned commit from the superproject index.
//   - Already initialized: the checkout is never moved. Updating it — after a
//     pin bump, or to a different branch/commit — is always a manual git step
//     (e.g. `git submodule update vite`, or a checkout inside `vite/`).
//
// (Pathspec is repo-root-relative, so run git from the repo root.)
export function ensureViteCheckout(): void {
  const isInitialized =
    nodeFs.existsSync(nodePath.join(viteDir, '.git')) &&
    nodeFs.existsSync(nodePath.join(viteDir, 'package.json'));
  if (!isInitialized) {
    // Full (non-shallow) clone: this Vite submodule is developed in-tree, so the
    // complete history is needed (branching, rebasing, blame, making commits). A
    // shallow `--depth 1` clone would leave the checkout grafted with no
    // ancestry — fine for a one-off build, but not for development.
    run('git submodule update --init vite', repoRoot);
  }
  const head = capture('git rev-parse HEAD', viteDir).slice(0, 12);
  const pinned = capture('git ls-files -s -- vite', repoRoot).split(/\s+/)[1];
  console.log(
    `[setup-vite] using vite ${head}` +
      (pinned.startsWith(head) ? '' : ` (superproject pins ${pinned.slice(0, 12)})`),
  );
}
