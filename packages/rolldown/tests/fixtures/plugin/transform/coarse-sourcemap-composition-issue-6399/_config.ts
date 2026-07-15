import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const loadedCode = `import { defineComponent } from 'vue';
export const ListFilter = /* @__PURE__ */ defineComponent({
  setup() {
    return () => {
      debugger;
    };
  }
});
`;

const transformedCode = `;
export const ListFilter = /* @__PURE__ */ Vue.defineComponent({
  setup() {
    return () => {
      debugger;
    };
  }
});
`;

function locate(code: string, text: string) {
  const offset = code.indexOf(text);
  expect(
    offset,
    `Could not find ${JSON.stringify(text)} in the generated chunk`,
  ).toBeGreaterThanOrEqual(0);
  const precedingLines = code.slice(0, offset).split('\n');
  return {
    line: precedingLines.length,
    column: precedingLines.at(-1)!.length,
  };
}

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'load-detailed-source-map',
        load(id) {
          if (!id.endsWith('/main.js')) {
            return;
          }
          return {
            code: loadedCode,
            map: {
              version: 3,
              names: ['defineComponent', 'ListFilter', 'setup'],
              sources: ['/src/index.tsx'],
              sourcesContent: [
                "import { defineComponent } from 'vue';\r\n\r\nexport const ListFilter = defineComponent({\r\n  setup() {\r\n    return () => {\r\n      debugger;\r\n    };\r\n  },\r\n});\r\n",
              ],
              mappings:
                'AAAA,SAASA,eAAe,QAAQ,KAAK;AAErC,OAAO,MAAMC,UAAU,GAAGD,+BAAe,CAAC;EACxCE,KAAKA,CAAA,EAAG;IACN,OAAO,MAAM;MACX;IACF,CAAC;EACH;AACF,CAAC,CAAC',
            },
          };
        },
      },
      {
        name: 'replace-define-component',
        transform(_code, id) {
          if (!id.endsWith('/main.js')) {
            return;
          }
          return {
            code: transformedCode,
            // This is MagicString's line-only map from the original reproduction.
            map: {
              version: 3,
              sources: [''],
              names: [],
              mappings: 'AAAqC;AACrC,0CAA0C,mBAAe;AACzD;AACA;AACA;AACA;AACA;AACA;',
            },
          };
        },
      },
    ],
  },
  async afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    const map = new TraceMap(JSON.parse(JSON.stringify(chunk.map)));
    const expectedMappings = [
      ['setup()', { line: 4, column: 2 }],
      ['return () =>', { line: 5, column: 4 }],
      ['debugger', { line: 6, column: 6 }],
    ] as const;

    for (const [statement, expected] of expectedMappings) {
      const original = originalPositionFor(map, locate(chunk.code, statement));
      expect(
        original,
        `Incorrect original position for ${JSON.stringify(statement)}`,
      ).toMatchObject({
        source: expect.stringMatching(/src\/index\.tsx$/),
        ...expected,
      });
    }
  },
});
