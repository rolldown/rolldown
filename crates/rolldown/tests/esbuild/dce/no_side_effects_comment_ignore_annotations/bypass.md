# Reason
1. rollup will print `#__NO_SIDE_EFFECTS__` even `tree.annotations: false`, we align with rollup https://rollupjs.org/repl/?version=4.27.3&shareable=JTdCJTIyZXhhbXBsZSUyMiUzQW51bGwlMkMlMjJtb2R1bGVzJTIyJTNBJTVCJTdCJTIyY29kZSUyMiUzQSUyMngoJTVCJTVDbiU1Q3QlMkYqJTIwJTIzX19OT19TSURFX0VGRkVDVFNfXyUyMColMkYlMjB5JTIwJTNEJTNFJTIweSUyQyU1Q24lNUN0JTJGKiUyMCUyM19fTk9fU0lERV9FRkZFQ1RTX18lMjAqJTJGJTIwKCklMjAlM0QlM0UlMjAlN0IlN0QlMkMlNUNuJTVDdCUyRiolMjAlMjNfX05PX1NJREVfRUZGRUNUU19fJTIwKiUyRiUyMCh5KSUyMCUzRCUzRSUyMCh5KSUyQyU1Q24lNUN0JTJGKiUyMCUyM19fTk9fU0lERV9FRkZFQ1RTX18lMjAqJTJGJTIwYXN5bmMlMjB5JTIwJTNEJTNFJTIweSUyQyU1Q24lNUN0JTJGKiUyMCUyM19fTk9fU0lERV9FRkZFQ1RTX18lMjAqJTJGJTIwYXN5bmMlMjAoKSUyMCUzRCUzRSUyMCU3QiU3RCUyQyU1Q24lNUN0JTJGKiUyMCUyM19fTk9fU0lERV9FRkZFQ1RTX18lMjAqJTJGJTIwYXN5bmMlMjAoeSklMjAlM0QlM0UlMjAoeSklMkMlNUNuJTVEKSU1Q24lMjIlMkMlMjJpc0VudHJ5JTIyJTNBdHJ1ZSUyQyUyMm5hbWUlMjIlM0ElMjJtYWluLmpzJTIyJTdEJTJDJTdCJTIyY29kZSUyMiUzQSUyMmNvbnNvbGUubG9nKCd0ZXN0JyklMjIlMkMlMjJpc0VudHJ5JTIyJTNBZmFsc2UlMkMlMjJuYW1lJTIyJTNBJTIycXV4LmpzJTIyJTdEJTJDJTdCJTIyY29kZSUyMiUzQSUyMiU3QiU1Q24lMjAlMjAlNUMlMjJzaWRlRWZmZWN0cyU1QyUyMiUzQSUyMGZhbHNlJTVDbiU3RCUyMiUyQyUyMmlzRW50cnklMjIlM0FmYWxzZSUyQyUyMm5hbWUlMjIlM0ElMjJwYWNrYWdlLmpzb24lMjIlN0QlNUQlMkMlMjJvcHRpb25zJTIyJTNBJTdCJTIyb3V0cHV0JTIyJTNBJTdCJTIyZm9ybWF0JTIyJTNBJTIyZXMlMjIlN0QlMkMlMjJ0cmVlc2hha2UlMjIlM0ElN0IlMjJhbm5vdGF0aW9ucyUyMiUzQXRydWUlN0QlN0QlN0Q=
# Diff
## /out/expr-fn.js
### esbuild
```js
x([
  function() {
  },
  function y() {
  },
  function* () {
  },
  function* y2() {
  },
  async function() {
  },
  async function y3() {
  },
  async function* () {
  },
  async function* y4() {
  }
]);
```
### rolldown
```js

//#region expr-fn.js
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
## /out/stmt-fn.js
### esbuild
```js
function a() {
}
function* b() {
}
async function c() {
}
async function* d() {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-fn.js
+++ rolldown	stmt-fn.js
@@ -1,4 +0,0 @@
-function a() {}
-function* b() {}
-async function c() {}
-async function* d() {}

```
## /out/stmt-export-fn.js
### esbuild
```js
export function a() {
}
export function* b() {
}
export async function c() {
}
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
var v0 = function() {
}, v1 = function() {
};
let l0 = function() {
}, l1 = function() {
};
const c0 = function() {
}, c1 = function() {
};
var v2 = () => {
}, v3 = () => {
};
let l2 = () => {
}, l3 = () => {
};
const c2 = () => {
}, c3 = () => {
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/stmt-local.js
+++ rolldown	stmt-local.js
@@ -1,6 +0,0 @@
-var v0 = function () {}, v1 = function () {};
-let l0 = function () {}, l1 = function () {};
-const c0 = function () {}, c1 = function () {};
-var v2 = () => {}, v3 = () => {};
-let l2 = () => {}, l3 = () => {};
-const c2 = () => {}, c3 = () => {};

```
## /out/stmt-export-local.js
### esbuild
```js
export var v0 = function() {
}, v1 = function() {
};
export let l0 = function() {
}, l1 = function() {
};
export const c0 = function() {
}, c1 = function() {
};
export var v2 = () => {
}, v3 = () => {
};
export let l2 = () => {
}, l3 = () => {
};
export const c2 = () => {
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
  function a() {
  }
  ns2.a = a;
  function* b() {
  }
  ns2.b = b;
  async function c() {
  }
  ns2.c = c;
  async function* d() {
  }
  ns2.d = d;
})(ns || (ns = {}));
```
### rolldown
```js

//#region ns-export-fn.ts
let ns;
(function(_ns) {
	/* @__NO_SIDE_EFFECTS__ */
	function a() {}
	_ns.a = a;
	/* @__NO_SIDE_EFFECTS__ */
	function* b() {}
	_ns.b = b;
	/* @__NO_SIDE_EFFECTS__ */
	async function c() {}
	_ns.c = c;
	/* @__NO_SIDE_EFFECTS__ */
	async function* d() {}
	_ns.d = d;
})(ns || (ns = {}));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/ns-export-fn.js
+++ rolldown	ns-export-fn.js
@@ -1,11 +1,11 @@
 var ns;
-(ns2 => {
+(function (_ns) {
     function a() {}
-    ns2.a = a;
+    _ns.a = a;
     function* b() {}
-    ns2.b = b;
+    _ns.b = b;
     async function c() {}
-    ns2.c = c;
+    _ns.c = c;
     async function* d() {}
-    ns2.d = d;
+    _ns.d = d;
 })(ns || (ns = {}));

```
## /out/ns-export-local.js
### esbuild
```js
var ns;
((ns2) => {
  ns2.v0 = function() {
  };
  ns2.v1 = function() {
  };
  ns2.l0 = function() {
  };
  ns2.l1 = function() {
  };
  ns2.c0 = function() {
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
  ns2.c2 = () => {
  };
  ns2.c3 = () => {
  };
})(ns || (ns = {}));
```
### rolldown
```js

//#region ns-export-local.ts
let ns;
(function(_ns) {
	var v0 = _ns.v0 = function() {}, v1 = _ns.v1 = function() {};
	let l0 = _ns.l0 = function() {}, l1 = _ns.l1 = function() {};
	const c0 = _ns.c0 = /* @__NO_SIDE_EFFECTS__ */ function() {}, c1 = _ns.c1 = function() {};
	var v2 = _ns.v2 = () => {}, v3 = _ns.v3 = () => {};
	let l2 = _ns.l2 = () => {}, l3 = _ns.l3 = () => {};
	const c2 = _ns.c2 = /* @__NO_SIDE_EFFECTS__ */ () => {}, c3 = _ns.c3 = () => {};
})(ns || (ns = {}));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/ns-export-local.js
+++ rolldown	ns-export-local.js
@@ -1,15 +1,9 @@
 var ns;
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
+(function (_ns) {
+    var v0 = _ns.v0 = function () {}, v1 = _ns.v1 = function () {};
+    let l0 = _ns.l0 = function () {}, l1 = _ns.l1 = function () {};
+    const c0 = _ns.c0 = function () {}, c1 = _ns.c1 = function () {};
+    var v2 = _ns.v2 = () => {}, v3 = _ns.v3 = () => {};
+    let l2 = _ns.l2 = () => {}, l3 = _ns.l3 = () => {};
+    const c2 = _ns.c2 = () => {}, c3 = _ns.c3 = () => {};
 })(ns || (ns = {}));

```
## /out/stmt-export-default-before-fn-anon.js
### esbuild
```js
export default function() {
}
```
### rolldown
```js

//#region stmt-export-default-before-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function* () {
}
```
### rolldown
```js

//#region stmt-export-default-before-gen-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-gen-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function* () {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-gen-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-before-async-gen-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function() {
}
```
### rolldown
```js

//#region stmt-export-default-after-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function* () {
}
```
### rolldown
```js

//#region stmt-export-default-after-gen-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-gen-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function* () {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-gen-fn-anon.js
/* @__NO_SIDE_EFFECTS__ */
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
export default async function* f() {
}
```
### rolldown
```js

//#region stmt-export-default-after-async-gen-fn-name.js
/* @__NO_SIDE_EFFECTS__ */
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