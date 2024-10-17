# Reason
1. require custom resolver
# Diff
## /out.js
### esbuild
```js
var test = 123;
var test_default = { test, "invalid-identifier": true };
export {
  test_default as default,
  test
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var test = 123;
-var test_default = {
-    test,
-    "invalid-identifier": true
-};
-export {test_default as default, test};

```