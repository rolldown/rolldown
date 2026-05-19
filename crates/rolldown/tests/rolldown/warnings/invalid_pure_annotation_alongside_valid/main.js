// Two annotations in one file: the first is applied (valid), the second is at an invalid position.
// Only the invalid one should produce a warning.
const a = /* #__PURE__ */ (() => 1)();
/* #__PURE__ */ globalThis.foo;
console.log(a);
