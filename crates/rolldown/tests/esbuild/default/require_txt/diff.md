# Diff
## /out.js
### esbuild
```js
// test.txt
var require_test = __commonJS({
  "test.txt"(exports, module) {
    module.exports = "This is a test.";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.txt
var test_exports, test_default;
var init_test = __esm({ "test.txt"() {
	test_exports = {};
	__export(test_exports, { default: () => test_default });
	test_default = "This is a test.";
} });

//#endregion
//#region entry.js
console.log((init_test(), __toCommonJS(test_exports)));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,11 @@
-var require_test = __commonJS({
-    "test.txt"(exports, module) {
-        module.exports = "This is a test.";
+var test_exports, test_default;
+var init_test = __esm({
+    "test.txt"() {
+        test_exports = {};
+        __export(test_exports, {
+            default: () => test_default
+        });
+        test_default = "This is a test.";
     }
 });
-console.log(require_test());
+console.log((init_test(), __toCommonJS(test_exports)));

```