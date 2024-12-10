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
	test,
	"invalid-identifier": invalid_identifier
};

//#endregion
export { test_default as default, test, invalid_identifier as "invalid-identifier" };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	test.js
@@ -3,5 +3,5 @@
 var test_default = {
     test,
     "invalid-identifier": invalid_identifier
 };
-export {test_default as default, invalid_identifier as undefined, test};
+export {test_default as default, test, invalid_identifier as undefined};

```