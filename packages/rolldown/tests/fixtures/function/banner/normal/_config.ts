import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';
const banner = () => bannerTxt;

export default defineTest({
  config: {
    output: {
      banner,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(bannerTxt)).toBe(true);
  },
});
