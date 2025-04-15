# Reason
1. annotation codegen
# Diff
## /out/expr-fn.js
### esbuild
```js
//! These should all have "no side effects"
x([
  /* @__NO_SIDE_EFFECTS__ */ function() {
  },
  /* @__NO_SIDE_EFFECTS__ */ function y() {
  },
  /* @__NO_SIDE_EFFECTS__ */ function* () {
  },
  /* @__NO_SIDE_EFFECTS__ */ function* y2() {
  },
  /* @__NO_SIDE_EFFECTS__ */ async function() {
  },
  /* @__NO_SIDE_EFFECTS__ */ async function y3() {
  },
  /* @__NO_SIDE_EFFECTS__ */ async function* () {
  },
  /* @__NO_SIDE_EFFECTS__ */ async function* y4() {
  }
]);
```
### rolldown
```js

//#region expr-fn.js
//! These should all have "no side effects"
x([
	/* @__NO_SIDE_EFFECTS__ */ function() {},
	/* @__NO_SIDE_EFFECTS__ */ function y() {},
	/* @__NO_SIDE_EFFECTS__ */ function* () {},
	/* @__NO_SIDE_EFFECTS__ */ function* y() {},
	/* @__NO_SIDE_EFFECTS__ */ async function() {},
	/* @__NO_SIDE_EFFECTS__ */ async function y() {},
	/* @__NO_SIDE_EFFECTS__ */ async function* () {},
	/* @__NO_SIDE_EFFECTS__ */ async function* y() {}
]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/expr-fn.js
+++ rolldown	expr-fn.js
@@ -1,1 +1,1 @@
-x([function () {}, function y() {}, function* () {}, function* y2() {}, async function () {}, async function y3() {}, async function* () {}, async function* y4() {}]);
+x([function () {}, function y() {}, function* () {}, function* y() {}, async function () {}, async function y() {}, async function* () {}, async function* y() {}]);

```
## /out/stmt-export-fn.js
### esbuild
```js
//! These should all have "no side effects"
// @__NO_SIDE_EFFECTS__
export function a() {
}
// @__NO_SIDE_EFFECTS__
export function* b() {
}
// @__NO_SIDE_EFFECTS__
export async function c() {
}
// @__NO_SIDE_EFFECTS__
export async function* d() {
}
```
### rolldown
```js

//#region stmt-export-fn.js
/* @__NO_SIDE_EFFECTS__ */
function a() {}
/* @__NO_SIDE_EFFECTS__ */
function* b() {}
/* @__NO_SIDE_EFFECTS__ */
async function c() {}
/* @__NO_SIDE_EFFECTS__ */
async function* d() {}

//#endregion
export { a, b, c, d };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-fn.js
+++ rolldown	stmt-export-fn.js
@@ -1,4 +1,5 @@
-export function a() {}
-export function* b() {}
-export async function c() {}
-export async function* d() {}
+function a() {}
+function* b() {}
+async function c() {}
+async function* d() {}
+export {a, b, c, d};

```
## /out/stmt-local.js
### esbuild
```js
//! Only "c0" and "c2" should have "no side effects" (Rollup only respects "const" and only for the first one)
var v0 = function() {
}, v1 = function() {
};
let l0 = function() {
}, l1 = function() {
};
const c0 = /* @__NO_SIDE_EFFECTS__ */ function() {
}, c1 = function() {
};
var v2 = () => {
}, v3 = () => {
};
let l2 = () => {
}, l3 = () => {
};
const c2 = /* @__NO_SIDE_EFFECTS__ */ () => {
}, c3 = () => {
};
```
### rolldown
```js

//#region stmt-local.js
//! Only "c0" and "c2" should have "no side effects" (Rollup only respects "const" and only for the first one)
var v0 = function() {}, v1 = function() {};
let l0 = function() {}, l1 = function() {};
const c0 = /* @__NO_SIDE_EFFECTS__ */ function() {}, c1 = function() {};
var v2 = () => {}, v3 = () => {};
let l2 = () => {}, l3 = () => {};
const c2 = /* @__NO_SIDE_EFFECTS__ */ () => {}, c3 = () => {};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-local.js
+++ rolldown	stmt-local.js
@@ -1,6 +1,6 @@
 var v0 = function () {}, v1 = function () {};
-let l0 = function () {}, l1 = function () {};
-const c0 = function () {}, c1 = function () {};
+var l0 = function () {}, l1 = function () {};
+var c0 = function () {}, c1 = function () {};
 var v2 = () => {}, v3 = () => {};
-let l2 = () => {}, l3 = () => {};
-const c2 = () => {}, c3 = () => {};
+var l2 = () => {}, l3 = () => {};
+var c2 = () => {}, c3 = () => {};

```
## /out/stmt-export-local.js
### esbuild
```js
//! Only "c0" and "c2" should have "no side effects" (Rollup only respects "const" and only for the first one)
export var v0 = function() {
}, v1 = function() {
};
export let l0 = function() {
}, l1 = function() {
};
export const c0 = /* @__NO_SIDE_EFFECTS__ */ function() {
}, c1 = function() {
};
export var v2 = () => {
}, v3 = () => {
};
export let l2 = () => {
}, l3 = () => {
};
export const c2 = /* @__NO_SIDE_EFFECTS__ */ () => {
}, c3 = () => {
};
```
### rolldown
```js

//#region stmt-export-local.js
var v0 = function() {};
var v1 = function() {};
let l0 = function() {};
let l1 = function() {};
const c0 = /* @__NO_SIDE_EFFECTS__ */ function() {};
const c1 = function() {};
var v2 = () => {};
var v3 = () => {};
let l2 = () => {};
let l3 = () => {};
const c2 = /* @__NO_SIDE_EFFECTS__ */ () => {};
const c3 = () => {};

//#endregion
export { c0, c1, c2, c3, l0, l1, l2, l3, v0, v1, v2, v3 };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-local.js
+++ rolldown	stmt-export-local.js
@@ -1,6 +1,13 @@
-export var v0 = function () {}, v1 = function () {};
-export let l0 = function () {}, l1 = function () {};
-export const c0 = function () {}, c1 = function () {};
-export var v2 = () => {}, v3 = () => {};
-export let l2 = () => {}, l3 = () => {};
-export const c2 = () => {}, c3 = () => {};
+var v0 = function () {};
+var v1 = function () {};
+var l0 = function () {};
+var l1 = function () {};
+var c0 = function () {};
+var c1 = function () {};
+var v2 = () => {};
+var v3 = () => {};
+var l2 = () => {};
+var l3 = () => {};
+var c2 = () => {};
+var c3 = () => {};
+export {c0, c1, c2, c3, l0, l1, l2, l3, v0, v1, v2, v3};

```
## /out/ns-export-fn.js
### esbuild
```js
var ns;
((ns2) => {
  //! These should all have "no side effects"
  // @__NO_SIDE_EFFECTS__
  function a() {
  }
  ns2.a = a;
  // @__NO_SIDE_EFFECTS__
  function* b() {
  }
  ns2.b = b;
  // @__NO_SIDE_EFFECTS__
  async function c() {
  }
  ns2.c = c;
  // @__NO_SIDE_EFFECTS__
  async function* d() {
  }
  ns2.d = d;
})(ns || (ns = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ns-export-fn.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var ns;
-(ns2 => {
-    function a() {}
-    ns2.a = a;
-    function* b() {}
-    ns2.b = b;
-    async function c() {}
-    ns2.c = c;
-    async function* d() {}
-    ns2.d = d;
-})(ns || (ns = {}));

```
## /out/ns-export-local.js
### esbuild
```js
var ns;
((ns2) => {
  //! Only "c0" and "c2" should have "no side effects" (Rollup only respects "const" and only for the first one)
  ns2.v0 = function() {
  };
  ns2.v1 = function() {
  };
  ns2.l0 = function() {
  };
  ns2.l1 = function() {
  };
  ns2.c0 = /* @__NO_SIDE_EFFECTS__ */ function() {
  };
  ns2.c1 = function() {
  };
  ns2.v2 = () => {
  };
  ns2.v3 = () => {
  };
  ns2.l2 = () => {
  };
  ns2.l3 = () => {
  };
  ns2.c2 = /* @__NO_SIDE_EFFECTS__ */ () => {
  };
  ns2.c3 = () => {
  };
})(ns || (ns = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ns-export-local.js
+++ rolldown	
@@ -1,15 +0,0 @@
-var ns;
-(ns2 => {
-    ns2.v0 = function () {};
-    ns2.v1 = function () {};
-    ns2.l0 = function () {};
-    ns2.l1 = function () {};
-    ns2.c0 = function () {};
-    ns2.c1 = function () {};
-    ns2.v2 = () => {};
-    ns2.v3 = () => {};
-    ns2.l2 = () => {};
-    ns2.l3 = () => {};
-    ns2.c2 = () => {};
-    ns2.c3 = () => {};
-})(ns || (ns = {}));

```
## /out/stmt-export-default-before-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function() {
}
```
### rolldown
```js

//#region stmt-export-default-before-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function stmt_export_default_before_fn_anon_default() {}

//#endregion
export { stmt_export_default_before_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-fn-anon.js
+++ rolldown	stmt-export-default-before-fn-anon.js
@@ -1,1 +1,2 @@
-export default function () {}
+function stmt_export_default_before_fn_anon_default() {}
+export {stmt_export_default_before_fn_anon_default as default};

```
## /out/stmt-export-default-before-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-fn-name.js
+++ rolldown	stmt-export-default-before-fn-name.js
@@ -1,1 +1,2 @@
-export default function f() {}
+function f() {}
+export {f as default};

```
## /out/stmt-export-default-before-gen-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function* () {
}
```
### rolldown
```js

//#region stmt-export-default-before-gen-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function* stmt_export_default_before_gen_fn_anon_default() {}

//#endregion
export { stmt_export_default_before_gen_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-gen-fn-anon.js
+++ rolldown	stmt-export-default-before-gen-fn-anon.js
@@ -1,1 +1,2 @@
-export default function* () {}
+function* stmt_export_default_before_gen_fn_anon_default() {}
+export {stmt_export_default_before_gen_fn_anon_default as default};

```
## /out/stmt-export-default-before-gen-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-gen-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function* f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-gen-fn-name.js
+++ rolldown	stmt-export-default-before-gen-fn-name.js
@@ -1,1 +1,2 @@
-export default function* f() {}
+function* f() {}
+export {f as default};

```
## /out/stmt-export-default-before-async-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function stmt_export_default_before_async_fn_anon_default() {}

//#endregion
export { stmt_export_default_before_async_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-async-fn-anon.js
+++ rolldown	stmt-export-default-before-async-fn-anon.js
@@ -1,1 +1,2 @@
-export default async function () {}
+async function stmt_export_default_before_async_fn_anon_default() {}
+export {stmt_export_default_before_async_fn_anon_default as default};

```
## /out/stmt-export-default-before-async-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-async-fn-name.js
+++ rolldown	stmt-export-default-before-async-fn-name.js
@@ -1,1 +1,2 @@
-export default async function f() {}
+async function f() {}
+export {f as default};

```
## /out/stmt-export-default-before-async-gen-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function* () {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-gen-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function* stmt_export_default_before_async_gen_fn_anon_default() {}

//#endregion
export { stmt_export_default_before_async_gen_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-async-gen-fn-anon.js
+++ rolldown	stmt-export-default-before-async-gen-fn-anon.js
@@ -1,1 +1,2 @@
-export default async function* () {}
+async function* stmt_export_default_before_async_gen_fn_anon_default() {}
+export {stmt_export_default_before_async_gen_fn_anon_default as default};

```
## /out/stmt-export-default-before-async-gen-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-gen-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function* f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-before-async-gen-fn-name.js
+++ rolldown	stmt-export-default-before-async-gen-fn-name.js
@@ -1,1 +1,2 @@
-export default async function* f() {}
+async function* f() {}
+export {f as default};

```
## /out/stmt-export-default-after-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function() {
}
```
### rolldown
```js

//#region stmt-export-default-after-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function stmt_export_default_after_fn_anon_default() {}

//#endregion
export { stmt_export_default_after_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-fn-anon.js
+++ rolldown	stmt-export-default-after-fn-anon.js
@@ -1,1 +1,2 @@
-export default function () {}
+function stmt_export_default_after_fn_anon_default() {}
+export {stmt_export_default_after_fn_anon_default as default};

```
## /out/stmt-export-default-after-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-fn-name.js
+++ rolldown	stmt-export-default-after-fn-name.js
@@ -1,1 +1,2 @@
-export default function f() {}
+function f() {}
+export {f as default};

```
## /out/stmt-export-default-after-gen-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function* () {
}
```
### rolldown
```js

//#region stmt-export-default-after-gen-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function* stmt_export_default_after_gen_fn_anon_default() {}

//#endregion
export { stmt_export_default_after_gen_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-gen-fn-anon.js
+++ rolldown	stmt-export-default-after-gen-fn-anon.js
@@ -1,1 +1,2 @@
-export default function* () {}
+function* stmt_export_default_after_gen_fn_anon_default() {}
+export {stmt_export_default_after_gen_fn_anon_default as default};

```
## /out/stmt-export-default-after-gen-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-gen-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
function* f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-gen-fn-name.js
+++ rolldown	stmt-export-default-after-gen-fn-name.js
@@ -1,1 +1,2 @@
-export default function* f() {}
+function* f() {}
+export {f as default};

```
## /out/stmt-export-default-after-async-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function stmt_export_default_after_async_fn_anon_default() {}

//#endregion
export { stmt_export_default_after_async_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-async-fn-anon.js
+++ rolldown	stmt-export-default-after-async-fn-anon.js
@@ -1,1 +1,2 @@
-export default async function () {}
+async function stmt_export_default_after_async_fn_anon_default() {}
+export {stmt_export_default_after_async_fn_anon_default as default};

```
## /out/stmt-export-default-after-async-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-async-fn-name.js
+++ rolldown	stmt-export-default-after-async-fn-name.js
@@ -1,1 +1,2 @@
-export default async function f() {}
+async function f() {}
+export {f as default};

```
## /out/stmt-export-default-after-async-gen-fn-anon.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function* () {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-gen-fn-anon.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function* stmt_export_default_after_async_gen_fn_anon_default() {}

//#endregion
export { stmt_export_default_after_async_gen_fn_anon_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-async-gen-fn-anon.js
+++ rolldown	stmt-export-default-after-async-gen-fn-anon.js
@@ -1,1 +1,2 @@
-export default async function* () {}
+async function* stmt_export_default_after_async_gen_fn_anon_default() {}
+export {stmt_export_default_after_async_gen_fn_anon_default as default};

```
## /out/stmt-export-default-after-async-gen-fn-name.js
### esbuild
```js
/*! This should have "no side effects" */
// @__NO_SIDE_EFFECTS__
export default async function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-gen-fn-name.js
/*! This should have "no side effects" */ /* @__NO_SIDE_EFFECTS__ */
async function* f() {}

//#endregion
export { f as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-export-default-after-async-gen-fn-name.js
+++ rolldown	stmt-export-default-after-async-gen-fn-name.js
@@ -1,1 +1,2 @@
-export default async function* f() {}
+async function* f() {}
+export {f as default};

```