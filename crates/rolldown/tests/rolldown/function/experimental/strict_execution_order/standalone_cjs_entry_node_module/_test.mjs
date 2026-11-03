import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const bundlePath = fileURLToPath(new URL('./dist/main.js', import.meta.url));
const bundle = require(bundlePath);
const cli = spawnSync(process.execPath, [bundlePath], { encoding: 'utf8' });

assert.deepStrictEqual(
  {
    required: bundle,
    cli: {
      status: cli.status,
      state: JSON.parse(cli.stdout),
    },
  },
  {
    required: {
      filename: bundlePath,
      hasParent: true,
      isMain: false,
    },
    cli: {
      status: 0,
      state: {
        filename: bundlePath,
        parentIsNull: true,
        isMain: true,
      },
    },
  },
);
