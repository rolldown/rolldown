let n = 0;
module.exports = {
  bump() {
    n += 1;
    return n;
  },
  self() {
    return module.exports;
  },
};
globalThis.events.push('CJS body');
