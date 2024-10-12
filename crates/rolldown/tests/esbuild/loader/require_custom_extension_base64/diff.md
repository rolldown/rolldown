# Diff
## /out.js
### esbuild
```js
// test.custom
var require_test = __commonJS({
  "test.custom"(exports, module) {
    module.exports = "YQBigGP/ZA==";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.custom
var test_exports, test_default;
var init_test = __esm({ "test.custom"() {
	test_exports = {};
	__export(test_exports, { default: () => test_default });
	test_default = "YVx4MDBiXHg4MGNceEZGZA==";
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
-    "test.custom"(exports, module) {
-        module.exports = "YQBigGP/ZA==";
+var test_exports, test_default;
+var init_test = __esm({
+    "test.custom"() {
+        test_exports = {};
+        __export(test_exports, {
+            default: () => test_default
+        });
+        test_default = "YVx4MDBiXHg4MGNceEZGZA==";
     }
 });
-console.log(require_test());
+console.log((init_test(), __toCommonJS(test_exports)));

```