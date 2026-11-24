import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { viteBuildImportAnalysisPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export function defineImportAttributesTest(importAttributes?: boolean) {
  return defineTest({
    config: {
      input: '../main.js',
      external: ['external.json'],
      ...(importAttributes === undefined ? {} : { experimental: { importAttributes } }),
      plugins: [
        viteBuildImportAnalysisPlugin({
          preloadCode: '',
          insertPreload: false,
          renderBuiltUrl: false,
          isRelativeBase: false,
        }),
      ],
    },
    afterTest(output) {
      const [{ code }] = getOutputChunk(output);
      const attributes = importAttributes ? ' with { type: "json" }' : '';
      expect(code).toContain(`import data from "external.json"${attributes};`);
    },
  });
}
