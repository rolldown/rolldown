import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';

export default defineTest({
  config: {
    output: {
      postBanner: bannerTxt,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(bannerTxt)).toBe(true);
  },
});
