// Shared CJS dep: needed by BOOT (static import in main.js) AND by the heavy
// stack (CJS require in heavy-stack.cjs).
module.exports.decode = function decode(x) {
  return 'decoded:' + x;
};
