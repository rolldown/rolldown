# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports, module) {
    module.exports = function() {
      return 123;
    };
  }
});

// entry.js
var fn = require_foo();
console.log(fn());
```
### rolldown
```js
import * as assert from "node:assert";

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports, module) {
	module.exports = function() {
		return 123;
	};
} });

//#region entry.js
const fn = require_foo();
assert.equal(fn(), 123);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_foo = __commonJS({
     "foo.js"(exports, module) {
         module.exports = function () {
             return 123;

```