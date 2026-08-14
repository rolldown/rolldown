import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';
const banner = () => bannerTxt;
const postBannerTxt = '/* post-banner */';
const postBanner = () => postBannerTxt;

export default defineTest({
  config: {
    output: {
      banner,
      postBanner,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(postBannerTxt + '\n' + bannerTxt)).toBe(true);
  },
});
