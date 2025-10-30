import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const outroText = '/* outro test */\n';
const outro = () => outroText;

export default defineTest({
  config: {
    output: {
      outro,
    },
  },
  afterTest(output) {
    expect(output.output[0].code.includes(outroText)).toBe(true);
  },
});
