const hyphenated = 1;
const quoted = 2;
const backslash = 3;
const lineBreak = 4;

export {
  hyphenated as 'x-y',
  quoted as 'single\'double"quote',
  backslash as 'back\\slash',
  lineBreak as 'line\nbreak\r\u2028\u2029',
};
