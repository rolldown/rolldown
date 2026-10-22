module.exports = {
  then(resolve) {
    globalThis.thenCalled('module-exports');
    resolve({});
  },
};
