// These Symbol() calls should be tree-shaken since they're unused
const VOID = Symbol("p-void");
const unused1 = Symbol();
const unused2 = Symbol('test');

// This should also be tree-shaken
Symbol('unused-direct-call');

// This should be kept because it's used
const USED = Symbol('used');

export default USED;
