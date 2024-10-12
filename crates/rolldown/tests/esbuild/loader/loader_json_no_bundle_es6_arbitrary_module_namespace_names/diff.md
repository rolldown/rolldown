# Diff
## /out.js
### esbuild
```js
var test = 123;
var invalid_identifier = true;
var test_default = { test, "invalid-identifier": invalid_identifier };
export {
  test_default as default,
  invalid_identifier as "invalid-identifier",
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
@@ -1,7 +0,0 @@
-var test = 123;
-var invalid_identifier = true;
-var test_default = {
-    test,
-    "invalid-identifier": invalid_identifier
-};
-export {test_default as default, invalid_identifier as undefined, test};

```