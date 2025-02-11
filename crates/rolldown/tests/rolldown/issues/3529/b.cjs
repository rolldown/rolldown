"use strict";

const a = require("./a.cjs");

// Difference from `minimal`: this says `module.exports` instead of `exports`
Object.defineProperty(module.exports, "b", {
  enumerable: true,
  get: () => () => ({ a }),
});
