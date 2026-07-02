import { isWasiTest } from '@tests/runtime-flavor';
import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  // KNOWN: wasm/emnapi error boundary — the `this.error(...)` diagnostic
  // (`[plugin my-plugin] <id>:1:4` prefix + code frame) is not rendered on
  // the WASI binding and the structured PLUGIN_ERROR fields are lost.
  // See fixtures/misc/error/load/_config.ts.
  skip: isWasiTest,
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async transform(_code, id) {
          if (id.includes('main.js')) {
            return this.error('my-error', 4);
          }
        },
      },
    ],
  },
  catchError(e: any) {
    const id = join(import.meta.dirname, 'main.js');
    expect(e.message).toContain(`\
[plugin my-plugin] ${id}:1:4
RolldownError: my-error
1: xxx;
       ^
2: yyy;
3: zzz;
`);
    expect(e.errors[0]).toMatchObject({
      message: 'my-error',
      code: 'PLUGIN_ERROR',
      plugin: 'my-plugin',
      hook: 'transform',
      id,
    });
  },
});
