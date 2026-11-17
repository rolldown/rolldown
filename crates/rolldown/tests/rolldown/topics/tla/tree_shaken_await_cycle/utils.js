import DEV from 'fake-env';
let relative = (s) => s;
if (DEV) {
  const path = await import('node:path');
  relative = (s) => path.basename(s);
}
(globalThis.tlaOrder ??= []).push('utils');
export function clean(s) {
  return relative(s);
}
