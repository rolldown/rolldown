exports.value = 1;
exports.setValue = (value) => {
  exports.value = value;
};
// A non-configurable accessor never changes, so a copy can use its getter.
Object.defineProperty(exports, 'fixed', { get: () => 'fixed', enumerable: true });
// A configurable accessor that the module replaces later.
Object.defineProperty(exports, 'replaced', {
  get: () => 'before',
  enumerable: true,
  configurable: true,
});
exports.replace = () => {
  Object.defineProperty(exports, 'replaced', { get: () => 'after' });
};
