import { isThreadlessWasi } from '@tests/runtime-flavor';
import type { SourceMap } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const filter = { id: /main\.js$/ };
let getSavedMap: () => SourceMap;
let originalMap: SourceMap;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'save-context-and-return-null',
        transform: {
          filter,
          handler() {
            getSavedMap = () => this.getCombinedSourcemap();
            originalMap = getSavedMap();
            return null;
          },
        },
      },
      {
        name: 'read-after-null-and-return-result',
        transform: {
          filter,
          handler(code) {
            if (isThreadlessWasi) {
              // Threadless WASI releases per-module hook contexts as soon as the hook settles.
              expect(getSavedMap).toThrow(
                /Memory has been freed: this transform context's native data was eagerly released/,
              );
            } else {
              const map = getSavedMap();
              expect(map).toEqual(originalMap);
              expect(map.mappings).not.toBe('');
            }
            return { code, map: null };
          },
        },
      },
      {
        name: 'read-after-result',
        transform: {
          filter,
          handler() {
            expect(getSavedMap).toThrow(
              isThreadlessWasi
                ? /Memory has been freed: this transform context's native data was eagerly released/
                : /^TransformPluginContext: `getCombinedSourcemap` cannot be called after the sourcemap chain is released$/,
            );
          },
        },
      },
    ],
  },
});
