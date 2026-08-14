// Pure annotation between `foo` and `=`, detached from the RHS call — invalid position.
const foo /* #__PURE__ */ = (() => 1)();
console.log(foo);
