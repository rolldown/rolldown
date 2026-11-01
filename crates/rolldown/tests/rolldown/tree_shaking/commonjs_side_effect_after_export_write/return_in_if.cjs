if (typeof process !== 'undefined') {
  exports.unused = 1;
  if (typeof window === 'undefined') return;
}
globalThis.afterReturn = true;
