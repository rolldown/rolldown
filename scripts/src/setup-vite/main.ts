// Set up the Vite checkout (`vite/` at the repo root) so the browser-platform
// tests run on Vite's full bundle mode backed by the workspace's local
// rolldown, WITHOUT modifying anything in the Vite repo:
//
//   1. ensure the checkout is at the latest `rolldown-canary` rebased onto
//      `main` (a checkout taken over by the developer is built as-is, see
//      checkout.ts),
//   2. `pnpm install --frozen-lockfile` (no manifest or lockfile writes),
//   3. swap the `packages/vite/node_modules/rolldown` symlink to point at the
//      workspace's `packages/rolldown`,
//   4. build the `vite` package. Vite's browser client inlines rolldown's
//      `DevRuntime` (`rolldown/experimental/runtime`) at build time, so the
//      swap must be in place BEFORE this step: the runtime the browser runs
//      has to match the code the workspace rolldown emits for it.
//
// The Vite source files are never patched. The swap is undone by any
// `pnpm install` inside the checkout, so re-run this script after that (it is
// idempotent).
//
// Requires `packages/rolldown` to be built first (`just build-rolldown`).
//
// Usage: `just setup-vite` (or `vp run --filter @rolldown-internal/scripts setup-vite`)

import nodeFs from 'node:fs';
import { createRequire } from 'node:module';
import nodePath from 'node:path';
import { ensureViteCheckout, repoRoot, run, viteDir } from './checkout.js';

const localRolldownDir = nodePath.join(repoRoot, 'packages', 'rolldown');

// 0. The local rolldown must exist — the harness (and Vite, after the swap)
// loads it at runtime.
if (!nodeFs.existsSync(nodePath.join(localRolldownDir, 'dist', 'index.mjs'))) {
  console.error(
    '[setup-vite] packages/rolldown/dist is missing — run `just build-rolldown` first.',
  );
  process.exit(1);
}

// 1. Ensure `vite/` has an up-to-date checkout (see checkout.ts), then
// build whatever commit is checked out.
ensureViteCheckout();

// 2. Install Vite's workspace deps exactly as pinned upstream, via vp. It
// delegates to the checkout's pinned pnpm itself, so no pnpm needs to be
// installed separately. This also resets any previous symlink swap from
// step 3, so the swap below always starts from a clean install.
run('vp install --frozen-lockfile', viteDir);

// 3. Point Vite's `rolldown` resolution at the workspace package.
const vitePkgDir = nodePath.join(viteDir, 'packages', 'vite');
const linkPath = nodePath.join(vitePkgDir, 'node_modules', 'rolldown');
// Resolve through any symlinked `vite/` first, so the relative link is
// computed from the directory it actually lives in.
const target = nodePath.relative(nodeFs.realpathSync(nodePath.dirname(linkPath)), localRolldownDir);
const current = nodeFs.existsSync(linkPath) ? nodeFs.realpathSync(linkPath) : null;
if (current !== nodeFs.realpathSync(localRolldownDir)) {
  nodeFs.rmSync(linkPath, { recursive: true, force: true });
  // 'junction' sidesteps Windows' symlink privilege requirement (admin /
  // Developer Mode); on other platforms the type is ignored. Node resolves
  // the target to an absolute path itself when creating a junction.
  nodeFs.symlinkSync(target, linkPath, 'junction');
  console.log(`[setup-vite] linked ${linkPath} -> ${target}`);
} else {
  console.log('[setup-vite] rolldown symlink already points at the workspace package');
}

// 4. Build the vite package (dist/node + dist/client). This mirrors Vite's
// own `build` script minus the type build, which the tests do not need.
// The bundle step is invoked through the checkout's own `rolldown` bin, not
// `vp run`: `vp run` re-syncs node_modules with the lockfile first, which
// would silently undo the swap from step 3 and inline the pinned runtime.
// The bin shim resolves through `node_modules/rolldown`, so after the swap
// the workspace rolldown both bundles Vite and provides the inlined runtime.
nodeFs.rmSync(nodePath.join(vitePkgDir, 'dist'), { recursive: true, force: true });
run('./node_modules/.bin/rolldown --config rolldown.config.ts', vitePkgDir);

// 5. Verify the override took: resolving `rolldown` from the vite package must
// land inside the workspace copy. Failing loudly here beats silently running
// the tests against the npm-pinned rolldown.
const viteRequire = createRequire(nodePath.join(vitePkgDir, 'package.json'));
const resolvedRolldown = nodeFs.realpathSync(viteRequire.resolve('rolldown'));
if (!resolvedRolldown.startsWith(nodeFs.realpathSync(localRolldownDir) + nodePath.sep)) {
  console.error(
    `[setup-vite] vite resolves rolldown to ${resolvedRolldown}, not the workspace ` +
      'packages/rolldown — the override did not take.',
  );
  process.exit(1);
}

console.log('[setup-vite] done, vite/packages/vite/dist is ready');
