# Reason
1. Different hash asset name
2. Same content has different name
# Diff
## /dist/out.js
### esbuild
```js
// a/test.txt
var require_test = __commonJS({
  "a/test.txt"(exports, module) {
    module.exports = "./test-J7OMUXO3.txt";
  }
});

// b/test.txt
var require_test2 = __commonJS({
  "b/test.txt"(exports, module) {
    module.exports = "./test-J7OMUXO3.txt";
  }
});

// entry.js
console.log(
  require_test(),
  require_test2()
);
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region a/test.txt
var require_test$1 = __commonJS({ "a/test.txt"(exports, module) {
	module.exports = "assets/test-BrHGE6Cv.txt";
} });

//#region b/test.txt
var require_test = __commonJS({ "b/test.txt"(exports, module) {
	module.exports = "assets/test-1P-S1VxP.txt";
} });

//#region entry.js
console.log(require_test$1(), require_test());

```
### diff
```diff
===================================================================
--- esbuild	/dist/out.js
+++ rolldown	entry.js
@@ -1,11 +1,17 @@
-var require_test = __commonJS({
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var require_test$1 = __commonJS({
     "a/test.txt"(exports, module) {
-        module.exports = "./test-J7OMUXO3.txt";
+        module.exports = "assets/test-BrHGE6Cv.txt";
     }
 });
-var require_test2 = __commonJS({
+var require_test = __commonJS({
     "b/test.txt"(exports, module) {
-        module.exports = "./test-J7OMUXO3.txt";
+        module.exports = "assets/test-1P-S1VxP.txt";
     }
 });
-console.log(require_test(), require_test2());
+console.log(require_test$1(), require_test());

```