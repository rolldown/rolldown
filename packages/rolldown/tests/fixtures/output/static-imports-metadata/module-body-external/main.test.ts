import { rolldown } from 'rolldown';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';

test.each([
  { format: 'es', input: './namespace.js', emitted: /import \* as [\w$]+ from "ext"/ },
  { format: 'cjs', input: './require.cjs', emitted: /\brequire\("ext"\)/ },
] as const)(
  'reports external imports emitted in $format module bodies',
  async ({ format, input, emitted }) => {
    const bundle = await rolldown({
      input,
      cwd: import.meta.dirname,
      external: ['ext'],
      plugins: [
        {
          name: 'check-imports',
          renderChunk(code, chunk) {
            expect(code).toMatch(emitted);
            expect(chunk.imports).toStrictEqual(['ext']);
          },
        },
      ],
    });

    try {
      const output = await bundle.generate({ format });
      expect(getOutputChunk(output).map((chunk) => chunk.imports)).toStrictEqual([['ext']]);
    } finally {
      await bundle.close();
    }
  },
);
