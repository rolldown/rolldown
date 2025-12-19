import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const bannerTxt = '/* banner */';
const shebang = '#!/usr/bin/env node\n';

export default defineTest({
  config: {
    output: {
      postBanner: bannerTxt,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(shebang + bannerTxt)).toBe(true);
  },
});
