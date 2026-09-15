import nodePath from 'node:path';

import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// `src/timer-host.ts` installs the CurrentThread hosts through
// `@napi-rs/async-runtime`; the browser flavor is a call-site option, not a
// separate code path, so keep the host protocol external and assert the call
// the bundle makes.
async function bundleTimerHost(browserBuild: boolean): Promise<string> {
  const bundle = await rolldown({
    input: nodePath.resolve(import.meta.dirname, '../src/timer-host.ts'),
    external: [/binding\.cjs$/, '@napi-rs/async-runtime'],
    transform: {
      define: {
        'import.meta.browserBuild': String(browserBuild),
      },
    },
  });

  try {
    const output = await bundle.generate({ format: 'esm' });
    return output.output
      .filter((item) => item.type === 'chunk')
      .map((item) => item.code)
      .join('\n');
  } finally {
    await bundle.close();
  }
}

test('browser builds install the ABI-v4 CurrentThread task host without a timer host', async () => {
  const code = await bundleTimerHost(true);

  expect(code).toContain('installCurrentThreadHosts');
  // Browser timer support remains a separate capability decision.
  expect(code).toMatch(/installTimerHost:\s*(?:!true|false)/);
});

test('node builds install the CurrentThread timer host alongside the task host', async () => {
  const code = await bundleTimerHost(false);

  expect(code).toContain('installCurrentThreadHosts');
  expect(code).toMatch(/installTimerHost:\s*(?:!false|true)/);
});
