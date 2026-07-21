import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'emit-asset',
        load(id) {
          if (!id.endsWith('main.js')) return;
          const referenceId = this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: fs.readFileSync(path.join(import.meta.dirname, 'asset.txt')),
          });
          return `export const x = 'a' + import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'comma-expression',
        resolveFileUrl() {
          // The leading operand has side effects, so no peephole pass can drop it
          // and collapse the sequence. That keeps the parenthesization observable;
          // a foldable `1, 2` would constant-fold to `"a2"` and hide it.
          return `console.log('side effect'), 2`;
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    // Deliberate divergence from Rollup's behavior
    //
    // Rollup splices the returned code as text, producing
    //   const x = 'a' + console.log('side effect'), 2;
    // where the comma operator swallows the addition and `x` becomes `"aundefined"`.
    // Rolldown parses the code into an AST, so codegen re-parenthesizes by operator
    // precedence and the grouping the plugin wrote is preserved: `x` is `"a2"`.
    expect(chunk.code).toContain('"a" + (console.log(');
    expect(chunk.code).not.toContain('"a" + console.log(');
  },
});
