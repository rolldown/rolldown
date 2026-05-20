// Generality lock-in for the RuntimeHelper boundary: a CJS-classified
// module that triggers more than one oxc-runtime helper (class field +
// private field at target es2021 → `_defineProperty` AND
// `_classPrivateFieldInitSpec` + `_classPrivateFieldGet`) must have all
// helper require sites unwrapped to `.default`, not just `_defineProperty`.
await import('./dist/main.js');
