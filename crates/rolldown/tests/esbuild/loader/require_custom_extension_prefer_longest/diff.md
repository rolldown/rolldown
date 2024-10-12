# Diff
## /out.js
### esbuild
```js
// test.txt
var require_test = __commonJS({
  "test.txt"(exports, module) {
    module.exports = "test.txt";
  }
});

// test.base64.txt
var require_test_base64 = __commonJS({
  "test.base64.txt"(exports, module) {
    module.exports = "dGVzdC5iYXNlNjQudHh0";
  }
});

// entry.js
console.log(require_test(), require_test_base64());
```
### rolldown
```js


//#region test.txt
var test_exports, test_default;
var init_test = __esm({ "test.txt"() {
	test_exports = {};
	__export(test_exports, { default: () => test_default });
	test_default = "test.txt";
} });

//#endregion
//#region test.base64.txt
var test_base64_exports, test_base64_default;
var init_test_base64 = __esm({ "test.base64.txt"() {
	test_base64_exports = {};
	__export(test_base64_exports, { default: () => test_base64_default });
	test_base64_default = "dGVzdC5iYXNlNjQudHh0";
} });

//#endregion
//#region entry.js
console.log((init_test(), __toCommonJS(test_exports)), (init_test_base64(), __toCommonJS(test_base64_exports)));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,21 @@
-var require_test = __commonJS({
-    "test.txt"(exports, module) {
-        module.exports = "test.txt";
+var test_exports, test_default;
+var init_test = __esm({
+    "test.txt"() {
+        test_exports = {};
+        __export(test_exports, {
+            default: () => test_default
+        });
+        test_default = "test.txt";
     }
 });
-var require_test_base64 = __commonJS({
-    "test.base64.txt"(exports, module) {
-        module.exports = "dGVzdC5iYXNlNjQudHh0";
+var test_base64_exports, test_base64_default;
+var init_test_base64 = __esm({
+    "test.base64.txt"() {
+        test_base64_exports = {};
+        __export(test_base64_exports, {
+            default: () => test_base64_default
+        });
+        test_base64_default = "dGVzdC5iYXNlNjQudHh0";
     }
 });
-console.log(require_test(), require_test_base64());
+console.log((init_test(), __toCommonJS(test_exports)), (init_test_base64(), __toCommonJS(test_base64_exports)));

```