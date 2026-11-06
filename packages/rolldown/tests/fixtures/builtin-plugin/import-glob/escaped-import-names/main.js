const hyphenated = import.meta.glob('./dir/*.js', {
  import: 'x-y',
  eager: true,
});

const quoted = import.meta.glob('./dir/*.js', {
  import: 'single\'double"quote',
  eager: true,
});

const backslash = import.meta.glob('./dir/*.js', {
  import: 'back\\slash',
  eager: true,
});

const lineBreak = import.meta.glob('./dir/*.js', {
  import: 'line\nbreak\r\u2028\u2029',
  eager: true,
});

export { hyphenated, quoted, backslash, lineBreak };
