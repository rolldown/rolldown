import { expect } from 'vitest';
import { defineScenario } from './scenario';
import { FORCE_WASI, run } from './utils';

// The plain `rolldown` package, which carries no binding and must pick up the packed
// @rolldown/binding-wasm32-wasi instead of the copy its WebContainer fallback downloads.
defineScenario({
  overlay: 'tests/fixtures/node',
  subject: 'packed rolldown plus its WASI binding',
  build: `${FORCE_WASI} pnpm run build`,
  async verifyInstall(container) {
    // the published `rolldown` package carries no binding of its own, so the only way the build
    // can succeed is through the separately packed @rolldown/binding-wasm32-wasi
    const distLocal = await run(
      container,
      'ls node_modules/rolldown/dist/rolldown-binding.wasi.cjs',
    );
    expect(
      distLocal.exitCode,
      `rolldown/dist ships its own WASI binding, so the binding package is not under test:\n${distLocal.output}`,
    ).not.toBe(0);

    // `rolldown` does not declare the WASI binding, so the require inside its bundled
    // dist/shared/binding-*.mjs resolves through pnpm's hidden hoisted store at
    // node_modules/.pnpm/node_modules. Resolve from the package's real path, not the symlink.
    const resolved = await container.runCommand('node', [
      '-e',
      "const { createRequire } = require('module'); const { realpathSync } = require('fs'); const { dirname, join } = require('path'); const pkg = realpathSync(createRequire(join(process.cwd(), 'x.js')).resolve('rolldown/package.json')); console.log(createRequire(join(dirname(pkg), 'dist/shared/binding.mjs')).resolve('@rolldown/binding-wasm32-wasi'))",
    ]);
    expect(resolved).toContain('/.pnpm/');
    expect(resolved).toContain('@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs');
  },
});
