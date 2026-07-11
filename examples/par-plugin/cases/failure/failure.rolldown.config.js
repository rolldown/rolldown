import nodePath from 'node:path';
import { defineConfig } from 'rolldown';
import parallelFailurePlugin from '../../parallel-failure-plugin/index.js';

const mode = process.env.PAR_FAILURE_MODE;
if (mode !== 'init' && mode !== 'sync' && mode !== 'reject') {
  throw new Error('PAR_FAILURE_MODE must be init, sync, or reject');
}

export default defineConfig({
  logLevel: 'silent',
  input: nodePath.resolve(import.meta.dirname, './input.js'),
  plugins: [parallelFailurePlugin({ mode })],
});
