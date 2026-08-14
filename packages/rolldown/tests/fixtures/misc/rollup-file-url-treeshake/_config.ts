import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'svg-resolver',
        resolveId(source, importer) {
          if (source.endsWith('.svg')) {
            return path.resolve(path.dirname(importer!), source);
          }
        },
        load(id) {
          if (id.endsWith('.svg')) {
            const referenceId = this.emitFile({
              type: 'asset',
              name: path.basename(id),
              source: fs.readFileSync(id),
            });
            return (
              `import.meta.ROLLUP_FILE_URL_${referenceId}\n` +
              `import.meta?.['ROLLUP_FILE_URL_${referenceId}']\n` +
              `export default 0;`
            );
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    // `import.meta.ROLLUP_FILE_URL_*` is side effect free, so an unused reference to it
    // should be tree-shaken away instead of being left behind as a bare `new URL(...).href;`
    const code = output.output[0].code;
    expect(code).not.toContain('new URL');
    expect(code).toContain(`console.log("kept")`);
  },
});
