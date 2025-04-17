# Diff
## /out.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports) {
    exports.foo = 213;
  }
});

// a.js
x ? __require("a") : y ? require_b() : __require("c");
x ? y ? __require("a") : require_b() : __require(c);
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};
var __require = /* @__PURE__ */ ((x$1) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x$1, { get: (a, b) => (typeof require !== "undefined" ? require : a)[b] }) : x$1)(function(x$1) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x$1 + "\" in an environment that doesn't expose the `require` function.");
});

//#region b.js
var require_b = __commonJS({ "b.js"(exports) {
	exports.foo = 213;
} });

//#region a.js
x ? __require("a") : y ? require_b() : __require("c");
x ? y ? __require("a") : require_b() : __require(c);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,4 +1,16 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var __require = (x$1 => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x$1, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x$1)(function (x$1) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x$1 + "\" in an environment that doesn't expose the `require` function.");
+});
 var require_b = __commonJS({
     "b.js"(exports) {
         exports.foo = 213;
     }

```