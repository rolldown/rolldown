import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = path.join(__dirname, './main.js');

// A shebang added via the `banner` hook (with no `output.banner` option) must land
// on the first line. The addon driver used to prepend a spurious `\n`, which both
// pushed the shebang off line 1 and broke shebang detection.
export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        banner: () => '#!/usr/bin/env node',
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith('#!/usr/bin/env node')).toBe(true);
  },
});
