if (globalThis.__loadConditionalCjs) {
  require('./conditional-cjs-side-effect.cjs');
}

module.exports = {
  decorate: (value) => `[conditional:${value}]`,
};
