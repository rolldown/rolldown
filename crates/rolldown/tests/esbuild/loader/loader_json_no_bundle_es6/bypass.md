# Reason
1. sub optimal
2. should inline literal in json
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
@@ -1,6 +1,7 @@
 var test = 123;
+var key_1 = true;
 var test_default = {
     test,
-    "invalid-identifier": true
+    "invalid-identifier": key_1
 };
-export {test_default as default, test};
+export {test_default as default, key_1 as undefined, test};

```