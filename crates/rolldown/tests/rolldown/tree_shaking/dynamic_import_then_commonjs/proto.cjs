exports.__proto__ = {
  then(resolve) {
    globalThis.thenCalled('proto');
    resolve({});
  },
};
