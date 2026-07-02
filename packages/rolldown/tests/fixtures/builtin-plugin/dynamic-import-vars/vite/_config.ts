import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  // KNOWN: single-thread block_on hazard. The JS `resolver` below is awaited
  // under `block_on` in ViteDynamicImportVarsPlugin::transform (lib.rs:122);
  // on the CurrentThread runtime the transform runs on the (blocked) JS
  // thread, so the TSFN continuation can never fire -> hard deadlock that not
  // even the vitest timeout can interrupt.
  skip: isSingleThread,
  sequential: true,
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id
            .replace('@', path.resolve(import.meta.dirname, './mods/'))
            .replace('#', path.resolve(import.meta.dirname, '../../'));
        },
      }),
    ],
  },
  async afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type === 'chunk') {
        await expect(chunk.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'main.js.snap'),
        );
      }
    }
  },
});
