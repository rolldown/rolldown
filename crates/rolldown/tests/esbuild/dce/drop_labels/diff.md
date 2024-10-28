# Reason
1. drop label feature
2. low priority
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
keep_1: require("foo1");
DROP_1: require("bar1");
exports.bar = function() {
	if (x) DROP_2: require("foo2");
	if (y) keep_2: require("bar2");
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,6 @@
 keep_1: require("foo1");
+DROP_1: require("bar1");
 exports.bar = function () {
-    if (x) ;
+    if (x) DROP_2: require("foo2");
     if (y) keep_2: require("bar2");
 };

```