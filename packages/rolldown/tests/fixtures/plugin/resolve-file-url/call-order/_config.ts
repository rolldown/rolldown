import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const calls: string[] = [];

export default defineTest({
  config: {
    plugins: [
      {
        name: 'emit-assets',
        transform(code, id) {
          if (!code.includes('ROLLUP_FILE_URL_REF')) return;
          const name = path.basename(id, '.js');
          const referenceId = this.emitFile({
            type: 'asset',
            name: `${name}.txt`,
            source: name,
          });
          return code.replace('ROLLUP_FILE_URL_REF', `ROLLUP_FILE_URL_${referenceId}`);
        },
      },
      {
        name: 'observes-order',
        resolveFileUrl(args) {
          calls.push(path.basename(args.moduleId));
          return `'call-${calls.length}'`;
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunk = output.output.find((o) => o.type === 'chunk')!;
    // Modules are visited in execution order — `dep` before the `main` entry
    // that imports it — rather than module discovery order. A stateful hook
    // therefore sees dep first, and each occurrence receives the matching value.
    expect(calls).toEqual(['dep.js', 'main.js']);
    expect(chunk.code).toContain('const depUrl = "call-1"');
    expect(chunk.code).toContain('const mainUrl = "call-2"');
  },
});
