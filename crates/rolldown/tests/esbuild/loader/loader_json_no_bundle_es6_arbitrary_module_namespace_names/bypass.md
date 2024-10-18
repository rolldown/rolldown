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
const test = 123;
const key_1 = true;
var test_default = {
	test,
	"invalid-identifier": key_1
};

//#endregion
export { test_default as default, key_1 as 'invalid-identifier', test };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	test.js
@@ -1,7 +1,7 @@
 var test = 123;
-var invalid_identifier = true;
+var key_1 = true;
 var test_default = {
     test,
-    "invalid-identifier": invalid_identifier
+    "invalid-identifier": key_1
 };
-export {test_default as default, invalid_identifier as undefined, test};
+export {test_default as default, key_1 as undefined, test};

```