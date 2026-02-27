import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const footerTxt = '// footer test';
const footer = () => footerTxt;
const postFooterTxt = '// post footer test\n';
const postFooter = () => postFooterTxt;

export default defineTest({
  config: {
    output: {
      minify: false,
      footer,
      postFooter,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.endsWith(footerTxt + '\n' + postFooterTxt)).toBe(true);
  },
});
