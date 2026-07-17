// Handling of the Vite checkout (`vite/` at the repo root, gitignored), used
// by the dev-server test harness (`packages/test-dev-server`, see `main.ts`).
// It is a plain clone of vitejs/vite on the `rolldown-canary` branch rebased
// onto the latest `main`, the same code `packages/vite-tests` runs on, so
// both harnesses track the canary line instead of a pinned commit.

import { execSync } from 'node:child_process';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';

// scripts/src/setup-vite/checkout.ts sits three levels below the repo root.
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

// Ensure `vite/` has a checkout, and use that checkout exactly as-is.
//
//   - Missing (fresh clone / CI): clone the `rolldown-canary` branch and
//     rebase it onto `origin/main`, so canary-side fixes take effect right
//     away and the newest Vite changes surface incompatibilities early.
//   - Already present: the checkout is never moved. Updating it, to a newer
//     `rolldown-canary` or to any other commit, is always a manual git step
//     inside `vite/`.
export function ensureViteCheckout(): void {
  if (!nodeFs.existsSync(nodePath.join(viteDir, 'package.json'))) {
    run('git clone --branch rolldown-canary https://github.com/vitejs/vite.git vite', repoRoot);
    run('git rebase origin/main', viteDir);
  }
  const head = capture('git rev-parse --short HEAD', viteDir);
  console.log(`[setup-vite] using vite ${head}`);
}
