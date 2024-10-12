# Diff
## /out.js
### esbuild
```js
// test.json
var require_test = __commonJS({
  "test.json"(exports, module) {
    module.exports = {
      a: true,
      b: 123,
      c: [null]
    };
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.json
var test_exports, a, b, c, test_default;
var init_test = __esm({ "test.json"() {
	test_exports = {};
	__export(test_exports, {
		a: () => a,
		b: () => b,
		c: () => c,
		default: () => test_default
	});
	a = true;
	b = 123;
	c = [null];
	test_default = {
		a,
		b,
		c
	};
} });

//#endregion
//#region entry.js
console.log((init_test(), __toCommonJS(test_exports).default));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,21 @@
-var require_test = __commonJS({
-    "test.json"(exports, module) {
-        module.exports = {
-            a: true,
-            b: 123,
-            c: [null]
+var test_exports, a, b, c, test_default;
+var init_test = __esm({
+    "test.json"() {
+        test_exports = {};
+        __export(test_exports, {
+            a: () => a,
+            b: () => b,
+            c: () => c,
+            default: () => test_default
+        });
+        a = true;
+        b = 123;
+        c = [null];
+        test_default = {
+            a,
+            b,
+            c
         };
     }
 });
-console.log(require_test());
+console.log((init_test(), __toCommonJS(test_exports).default));

```