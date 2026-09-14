import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import type * as Binding from '../../src/binding.cjs';

export function loadBinding(): typeof Binding {
  const require = createRequire(import.meta.url);
  if (process.env.ROLLDOWN_WASI_TEST) {
    return require('../../dist/rolldown-binding.wasi.cjs');
  }

  const bindingFile = readdirSync(new URL('../../dist/', import.meta.url)).find((file) =>
    file.endsWith('.node'),
  );
  if (!bindingFile) {
    throw new Error('No native binding found in dist. Run `just build-rolldown` first.');
  }
  return require(`../../dist/${bindingFile}`);
}
