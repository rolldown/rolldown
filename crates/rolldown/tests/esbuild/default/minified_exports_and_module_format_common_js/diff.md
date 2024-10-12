# Diff
## /out.js
### esbuild
```js
// foo/test.js
var o = {};
p(o, {
  foo: () => l
});
var l = 123;

// bar/test.js
var r = {};
p(r, {
  bar: () => m
});
var m = 123;

// entry.js
console.log(exports, module.exports, o, r);
```
### rolldown
```js


//#region foo/test.js
var test_exports, foo;
var init_test$1 = __esm({ "foo/test.js"() {
	test_exports = {};
	__export(test_exports, { foo: () => foo });
	foo = 123;
} });

//#endregion
//#region bar/test.js
var test_exports$1, bar;
var init_test = __esm({ "bar/test.js"() {
	test_exports$1 = {};
	__export(test_exports$1, { bar: () => bar });
	bar = 123;
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	init_test$1();
	init_test();
	console.log(exports, module.exports, test_exports, test_exports$1);
} });

//#endregion
export default require_entry();


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,28 @@
-var o = {};
-p(o, {
-    foo: () => l
+var test_exports, foo;
+var init_test$1 = __esm({
+    "foo/test.js"() {
+        test_exports = {};
+        __export(test_exports, {
+            foo: () => foo
+        });
+        foo = 123;
+    }
 });
-var l = 123;
-var r = {};
-p(r, {
-    bar: () => m
+var test_exports$1, bar;
+var init_test = __esm({
+    "bar/test.js"() {
+        test_exports$1 = {};
+        __export(test_exports$1, {
+            bar: () => bar
+        });
+        bar = 123;
+    }
 });
-var m = 123;
-console.log(exports, module.exports, o, r);
+var require_entry = __commonJS({
+    "entry.js"(exports, module) {
+        init_test$1();
+        init_test();
+        console.log(exports, module.exports, test_exports, test_exports$1);
+    }
+});
+export default require_entry();

```