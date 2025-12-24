import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';
const postBanner = () => bannerTxt;

export default defineTest({
  config: {
    output: {
      postBanner,
      minify: true,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(bannerTxt)).toBe(true);
  },
});
