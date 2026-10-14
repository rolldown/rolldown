import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import type * as Binding from '../../src/binding.cjs';

export function loadBinding(): typeof Binding {
  const require = createRequire(import.meta.url);
  if (process.env.ROLLDOWN_WASI_TEST) {
    // The threadless lane ships `rolldown-binding.wasip1.cjs`; the threaded lane
    // ships `rolldown-binding.wasi.cjs`. Load whichever flavor dist holds.
    const wasiFile = readdirSync(new URL('../../dist/', import.meta.url)).find(
      (file) => file === 'rolldown-binding.wasip1.cjs' || file === 'rolldown-binding.wasi.cjs',
    );
    if (!wasiFile) {
      throw new Error('No WASI binding found in dist. Run `just build-rolldown-wasi` first.');
    }
    return require(`../../dist/${wasiFile}`);
  }

  const bindingFile = readdirSync(new URL('../../dist/', import.meta.url)).find((file) =>
    file.endsWith('.node'),
  );
  if (!bindingFile) {
    throw new Error('No native binding found in dist. Run `just build-rolldown` first.');
  }
  return require(`../../dist/${bindingFile}`);
}
