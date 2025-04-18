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
var test = 123;
var invalid_identifier = true;
var test_default = {
	test,
	"invalid-identifier": invalid_identifier
};

//#endregion
export { test_default as default, invalid_identifier as "invalid-identifier", test };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	test.js
@@ -1,6 +1,7 @@
 var test = 123;
+var invalid_identifier = true;
 var test_default = {
     test,
-    "invalid-identifier": true
+    "invalid-identifier": invalid_identifier
 };
-export {test_default as default, test};
+export {test_default as default, invalid_identifier as undefined, test};

```