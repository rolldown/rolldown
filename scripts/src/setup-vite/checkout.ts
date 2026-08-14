// Handling of the Vite checkout (`vite/` at the repo root, gitignored), the
// single Vite checkout shared by the dev-server test harness
// (`packages/test-dev-server`, see `main.ts`) and Vite's own test suite
// (`packages/vite-tests/run.ts`, which clones this checkout locally). It is
// a plain clone of vitejs/vite on the `rolldown-canary` branch rebased onto
// the latest `main`, so both harnesses track the canary line instead of a
// pinned commit.

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

// Ensure `vite/` has a checkout at the latest `rolldown-canary` rebased onto
// the latest `main`, so canary-side fixes take effect right away and the
// newest Vite changes surface incompatibilities early.
//
//   - Missing (fresh clone / CI): clone `rolldown-canary` and rebase it.
//   - Clean and on `rolldown-canary`: update to the latest upstream state
//     (kept as-is when the fetch fails, e.g. offline).
//   - Dirty or not on `rolldown-canary`: taken over by the developer, used
//     exactly as-is so local experiments are never trampled.
export function ensureViteCheckout(): void {
  if (!nodeFs.existsSync(nodePath.join(viteDir, 'package.json'))) {
    run('git clone --branch rolldown-canary https://github.com/vitejs/vite.git vite', repoRoot);
    run('git rebase origin/main', viteDir);
  } else {
    updateViteCheckout();
  }
  const head = capture('git rev-parse --short HEAD', viteDir);
  console.log(`[setup-vite] using vite ${head}`);
}

function updateViteCheckout(): void {
  const dirty = capture('git status --porcelain', viteDir) !== '';
  if (dirty || capture('git rev-parse --abbrev-ref HEAD', viteDir) !== 'rolldown-canary') {
    console.log(
      `[setup-vite] vite/ ${dirty ? 'has local changes' : 'is not on rolldown-canary'}, using it as-is`,
    );
    return;
  }
  try {
    run('git fetch origin main rolldown-canary', viteDir);
  } catch {
    console.log('[setup-vite] fetch failed, using the existing checkout as-is');
    return;
  }
  run('git checkout -B rolldown-canary origin/rolldown-canary', viteDir);
  run('git rebase origin/main', viteDir);
}
