"use strict";

const b = require("./b.cjs");

// Difference from `minimal`: this says `module.exports` instead of `exports`
Object.defineProperty(module.exports, "a", {
  enumerable: true,
  get: () => () => ({ b }),
});
