export function multiByteAfterLineBreak(name) {
  return import(`./dir/
é${name}.js`);
}

export function asciiAfterLineBreak(name) {
  return import(`./dir/
x/${name}.js`);
}
