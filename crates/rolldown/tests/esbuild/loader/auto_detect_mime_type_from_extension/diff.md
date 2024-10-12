# Diff
## /out.js
### esbuild
```js
// test.svg
var require_test = __commonJS({
  "test.svg"(exports, module) {
    module.exports = "data:image/svg+xml;base64,YQBigGP/ZA==";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.svg
var test_exports, test_default;
var init_test = __esm({ "test.svg"() {
	test_exports = {};
	__export(test_exports, { default: () => test_default });
	test_default = "data:image/svg+xml;base64,YQBigGP/ZA==";
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
-    "test.svg"(exports, module) {
-        module.exports = "data:image/svg+xml;base64,YQBigGP/ZA==";
+var test_exports, test_default;
+var init_test = __esm({
+    "test.svg"() {
+        test_exports = {};
+        __export(test_exports, {
+            default: () => test_default
+        });
+        test_default = "data:image/svg+xml;base64,YQBigGP/ZA==";
     }
 });
-console.log(require_test());
+console.log((init_test(), __toCommonJS(test_exports)));

```