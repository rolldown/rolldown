import nodePath from 'node:path';

import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

test('browser builds register the CurrentThread fresh-turn task host', async () => {
  const bundle = await rolldown({
    input: nodePath.resolve(import.meta.dirname, '../src/timer-host.ts'),
    external: [/binding\.cjs$/],
    transform: {
      define: {
        'import.meta.browserBuild': 'true',
      },
    },
  });

  try {
    const output = await bundle.generate({ format: 'esm' });
    const code = output.output
      .filter((item) => item.type === 'chunk')
      .map((item) => item.code)
      .join('\n');

    // Paired with the Rust Shared-wake regression: browser builds must retain
    // the host turn that prevents a scheduler wake from polling inline.
    expect(code).toContain('registerCurrentThreadTaskHost');
    expect(code).not.toContain('driveCurrentThreadRuntimeTasks');

    // Browser timer support remains a separate capability decision.
    expect(code).not.toContain('registerTimerHost(');
  } finally {
    await bundle.close();
  }
});
