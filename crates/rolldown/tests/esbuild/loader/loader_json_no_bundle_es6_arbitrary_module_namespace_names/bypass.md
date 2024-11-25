# Reason
1. different naming style
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

//#region test.json
var test = 123;
var invalid_identifier = true;
var test_default = {
	"test": test,
	"invalid-identifier": invalid_identifier
};

//#endregion
export { test_default as default, invalid_identifier as 'invalid-identifier', test };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	test.js
@@ -1,7 +1,7 @@
 var test = 123;
 var invalid_identifier = true;
 var test_default = {
-    test,
+    "test": test,
     "invalid-identifier": invalid_identifier
 };
 export {test_default as default, invalid_identifier as undefined, test};

```