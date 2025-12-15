import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';
const postBanner = async () => bannerTxt;

export default defineTest({
  config: {
    output: {
      postBanner,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(bannerTxt)).toBe(true);
  },
});
