// Pure annotation directly before a call expression — valid position, no warning.
const foo = /* #__PURE__ */ (() => 1)();
console.log(foo);
