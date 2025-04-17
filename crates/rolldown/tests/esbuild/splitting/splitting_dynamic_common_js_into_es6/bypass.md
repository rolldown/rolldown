# Reason
1. different chunk naming style
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-X6C7FV5C.js").then(({ default: { bar } }) => console.log(bar));
```
### rolldown
```js

//#region entry.js
import("./foo.js").then(({ default: { bar } }) => console.log(bar));

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-import("./foo-X6C7FV5C.js").then(({default: {bar}}) => console.log(bar));
+import("./foo.js").then(({default: {bar}}) => console.log(bar));

```
## /out/foo-X6C7FV5C.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});
export default require_foo();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.bar = 123;
} });

export default require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-X6C7FV5C.js
+++ rolldown	foo.js
@@ -1,4 +1,10 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.bar = 123;
     }

```