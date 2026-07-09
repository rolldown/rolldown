import nodePath from 'node:path';

import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

test('browser builds register the ABI-v2 CurrentThread task host', async () => {
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

    expect(code).toContain('getCurrentThreadTaskHostContractVersion');
    expect(code).toContain('registerCurrentThreadTaskHost');
    expect(code).not.toContain('driveCurrentThreadRuntimeTasks');
    expect(code).not.toContain('cancelCurrentThreadRuntimeTaskDispatch');

    // Browser timer support remains a separate capability decision.
    expect(code).not.toContain('registerTimerHost(');
  } finally {
    await bundle.close();
  }
});
