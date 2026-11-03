exports.then = function (resolve) {
  globalThis.droppedCjsThenCalls = (globalThis.droppedCjsThenCalls ?? 0) + 1;
  resolve({});
};
