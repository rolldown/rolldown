import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Regression test for https://github.com/rolldown/rolldown/issues/9748.
//
// Replaying warnings used to do one Rust -> JS -> Rust round-trip per warning,
// awaited sequentially. With tens of thousands of warnings that degenerated into
// a pathological ping-pong across the napi bridge that could appear to hang the
// build. Warnings are now emitted with bounded concurrency; this asserts that the
// pipelining still delivers every warning exactly once (none dropped/duplicated)
// and that a high-volume build completes.
test('delivers every warning when a build emits a high volume of them', async () => {
  const WARNING_COUNT = 5000;
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
