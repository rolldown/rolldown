import { spawnSync } from 'child_process';
import path from 'path';

spawnSync(process.execPath, [
  '--import',
  'tsx/esm',
  '-C',
  'dev',
  path.resolve(import.meta.dirname, 'node_modules/tsdown/dist/run.mjs'),
  ...process.argv.slice(2),
], { stdio: 'inherit' });
