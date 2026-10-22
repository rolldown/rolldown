exports.value = 'value';
this.then = function (resolve) {
  globalThis.thenCalled('this-write');
  resolve({});
};
