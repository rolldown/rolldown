const key = 'then';
exports[key] = function (resolve) {
  globalThis.thenCalled('variable-key');
  resolve({});
};
