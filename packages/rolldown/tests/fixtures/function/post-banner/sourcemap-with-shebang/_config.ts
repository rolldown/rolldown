import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      postBanner: '/* line 1 */\n/* line 2 */',
      sourcemap: true,
    },
    experimental: {
      attachDebugInfo: 'none',
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    const lines = chunk.code.split('\n');
    expect(lines[0]).toBe('#!/usr/bin/env node');
    expect(lines[1]).toBe('/* line 1 */');
    expect(lines[2]).toBe('/* line 2 */');

    const map = chunk.map!;
    expect(map).toBeDefined();
    expect(map.mappings).toMatchInlineSnapshot(`";;;AACA,QAAQ,IAAI,QAAQ"`);
    expect(map.sourcesContent[0]).toBe("#!/usr/bin/env node\nconsole.log('hello');\n");
  },
});
