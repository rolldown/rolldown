import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: 'preserve',
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code.replace(/\s+/g, '')).toBe(
      `//#regionmain.jsxconsole.log(<div>test</div>);//#endregion`,
    );
  },
});
