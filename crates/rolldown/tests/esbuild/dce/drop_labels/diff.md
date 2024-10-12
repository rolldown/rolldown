# Diff
## /out.js
### esbuild
```js
// entry.js
keep_1: require("foo1");
exports.bar = function() {
  if (x) ;
  if (y) keep_2: require("bar2");
};
```
### rolldown
```js


//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	keep_1: require("foo1");
	DROP_1: require("bar1");
	exports.bar = function() {
		if (x) DROP_2: require("foo2");
		if (y) keep_2: require("bar2");
	};
} });

//#endregion
export default require_entry();


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,11 @@
-keep_1: require("foo1");
-exports.bar = function () {
-    if (x) ;
-    if (y) keep_2: require("bar2");
-};
+var require_entry = __commonJS({
+    "entry.js"(exports) {
+        keep_1: require("foo1");
+        DROP_1: require("bar1");
+        exports.bar = function () {
+            if (x) DROP_2: require("foo2");
+            if (y) keep_2: require("bar2");
+        };
+    }
+});
+export default require_entry();

```