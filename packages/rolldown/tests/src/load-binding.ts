import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import type * as Binding from '../../src/binding.cjs';

export function loadBinding(): typeof Binding {
  return createRequire(import.meta.url)(getBindingPath());
}

export function getBindingPath(): string {
  if (process.env.ROLLDOWN_WASI_TEST) {
    return fileURLToPath(new URL('../../dist/rolldown-binding.wasi.cjs', import.meta.url));
  }

  const bindingFile = readdirSync(new URL('../../dist/', import.meta.url)).find((file) =>
    file.endsWith('.node'),
  );
  if (!bindingFile) {
    throw new Error('No native binding found in dist. Run `just build-rolldown` first.');
  }
  return fileURLToPath(new URL(`../../dist/${bindingFile}`, import.meta.url));
}
