import { createRequire } from 'node:module';
import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'cjs',
      esModule: true,
    },
  },
  afterTest: (output) => {
    fs.writeFileSync(path.join(import.meta.dirname, 'dist/package.json'), '{ "type": "commonjs" }');
    const require = createRequire(import.meta.url);
    expect(
      output.output
        .filter((output): output is RolldownOutputChunk => output.type === 'chunk' && output.isEntry)
        .every((chunk) =>
          require(`./dist/${chunk.fileName}`).__esModule
        ),
    ).toBe(true);
  },
});
