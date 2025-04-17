# Reason
1. `sub` is not resolved
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/pkg/sub/bar.js
var require_bar = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/bar.js"() {
    works();
  }
});

// Users/user/project/src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
    require_bar();
  }
});

// Users/user/project/src/entry.js
require_foo();
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

//#region src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({ "src/node_modules/pkg/sub/foo.js"() {
	__require("sub");
} });

//#region src/entry.js
require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,11 +1,18 @@
-var require_bar = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/bar.js"() {
-        works();
-    }
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
 });
 var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
-        require_bar();
+    "src/node_modules/pkg/sub/foo.js"() {
+        __require("sub");
     }
 });
 require_foo();

```