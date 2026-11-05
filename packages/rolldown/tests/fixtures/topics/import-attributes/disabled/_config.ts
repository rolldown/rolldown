import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const data = path.join(import.meta.dirname, 'data.json');
const calls: Record<string, unknown> = {};

// Without `experimental.importAttributes`, hooks see no attributes and they are not part of the
// module identity.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source, _importer, { attributes }) {
          if (source === './data.json') calls.resolveId = attributes;
        },
        load(id, { rawId, attributes }) {
          if (rawId === data) calls.load = { id, rawId, attributes };
        },
      },
    ],
  },
  afterTest() {
    expect(calls).toEqual({
      resolveId: {},
      load: { id: data, rawId: data, attributes: {} },
    });
  },
});
