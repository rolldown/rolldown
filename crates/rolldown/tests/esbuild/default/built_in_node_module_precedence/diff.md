# Diff
## /out/entry.js
### esbuild
```js
// node_modules/fs/abc.js
var require_abc = __commonJS({
  "node_modules/fs/abc.js"() {
    console.log("include this");
  }
});

// node_modules/fs/index.js
var require_fs = __commonJS({
  "node_modules/fs/index.js"() {
    console.log("include this too");
  }
});

// entry.js
console.log([
  // These are node core modules
  require("fs"),
  require("fs/promises"),
  require("node:foo"),
  // These are not node core modules
  require_abc(),
  require_fs()
]);
```
### rolldown
```js
//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};


//#region node_modules/fs/abc.js
var require_abc = __commonJS({ "node_modules/fs/abc.js"() {
	console.log("include this");
} });

//#region node_modules/fs/index.js
var require_fs = __commonJS({ "node_modules/fs/index.js"() {
	console.log("include this too");
} });

//#region entry.js
console.log([
	require("fs"),
	require("fs/promises"),
	require("node:foo"),
	require_abc(),
	require_fs()
]);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_abc = __commonJS({
     "node_modules/fs/abc.js"() {
         console.log("include this");
     }

```