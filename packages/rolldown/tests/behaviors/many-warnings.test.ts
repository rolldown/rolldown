import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Regression test for https://github.com/rolldown/rolldown/issues/9748.
//
// Emitting a high volume of warnings used to be O(N^2): for every warning,
// rolldown re-scanned the source from offset 0 to locate the span and rebuilt
// the ariadne `Source` (line index) for the whole file. With tens of thousands
// of warnings in one large module that made the build appear to hang. The
// per-source work is now shared across all diagnostics (O(N log N)), so even 20k
// warnings finish in well under a second. This asserts both that the build
// completes and that every warning is delivered exactly once (none dropped or
// duplicated).
test('delivers every warning when a build emits a high volume of them', async () => {
  const WARNING_COUNT = 20000;
  const virtualId = '\0many-eval-warnings';

  let evalWarnings = 0;
  const bundle = await rolldown({
    input: virtualId,
    plugins: [
      {
        name: 'virtual-eval',
        resolveId(id) {
          if (id === virtualId) return id;
        },
        load(id) {
          if (id === virtualId) {
            // Each direct `eval(...)` call emits one EVAL warning during scan.
            let src = '';
            for (let i = 0; i < WARNING_COUNT; i++) src += `eval("${i}");\n`;
            return src;
          }
        },
      },
    ],
    onwarn(warning) {
      if (warning.code === 'EVAL') evalWarnings++;
    },
  });
  await bundle.generate({});
  await bundle.close();

  expect(evalWarnings).toBe(WARNING_COUNT);
});
