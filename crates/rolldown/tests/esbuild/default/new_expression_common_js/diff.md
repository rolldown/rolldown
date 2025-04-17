# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports, module) {
    var Foo = class {
    };
    module.exports = { Foo };
  }
});

// entry.js
new (require_foo()).Foo();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports, module) {
	var Foo = class {};
	module.exports = { Foo };
} });

//#region entry.js
new (require_foo()).Foo();

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
         var Foo = class {};
         module.exports = {

```