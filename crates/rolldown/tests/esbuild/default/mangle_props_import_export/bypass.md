# Reason
1. could be done in minifier
2. for `__require` diff, we don't have ModePassThrough
# Diff
## /out/esm.js
### esbuild
```js
export let foo_ = 123;
import { bar_ } from "xyz";
```
### rolldown
```js
import "xyz";

//#region esm.js
let foo_ = 123;

export { foo_ };
```
### diff
```diff
===================================================================
--- esbuild	/out/esm.js
+++ rolldown	esm.js
@@ -1,2 +1,3 @@
-export let foo_ = 123;
-import {bar_} from "xyz";
+import "xyz";
+var foo_ = 123;
+export {foo_};

```
## /out/cjs.js
### esbuild
```js
exports.a = 123;
let bar_ = require("xyz").b;
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, { get: (a, b) => (typeof require !== "undefined" ? require : a)[b] }) : x)(function(x) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
});

//#region cjs.js
var require_cjs = __commonJS({ "cjs.js"(exports) {
	exports.foo_ = 123;
	let bar_ = __require("xyz").bar_;
} });

export default require_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs.js
+++ rolldown	cjs.js
@@ -1,2 +1,19 @@
-exports.a = 123;
-let bar_ = require("xyz").b;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
+});
+var require_cjs = __commonJS({
+    "cjs.js"(exports) {
+        exports.foo_ = 123;
+        let bar_ = __require("xyz").bar_;
+    }
+});
+export default require_cjs();

```