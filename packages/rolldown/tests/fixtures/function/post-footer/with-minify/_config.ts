import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const footerTxt = '// footer test\n';
const postFooter = () => footerTxt;

export default defineTest({
  config: {
    output: {
      postFooter,
      minify: true,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.endsWith(footerTxt)).toBe(true);
  },
});
