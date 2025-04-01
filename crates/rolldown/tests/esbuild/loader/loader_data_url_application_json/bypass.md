# Reason
1. different naming style
# Diff
## /out/entry.js
### esbuild
```js
// <data:application/json,"%31%32%33">
var json_31_32_33_default = "123";

// <data:application/json;base64,eyJ3b3JrcyI6dHJ1ZX0=>
var json_base64_eyJ3b3JrcyI6dHJ1ZX0_default = { works: true };

// <data:application/json;charset=UTF-8,%31%32%33>
var json_charset_UTF_8_31_32_33_default = 123;

// <data:application/json;charset=UTF-8;base64,eyJ3b3JrcyI6dHJ1ZX0=>
var json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0_default = { works: true };

// entry.js
console.log([
  json_31_32_33_default,
  json_base64_eyJ3b3JrcyI6dHJ1ZX0_default,
  json_charset_UTF_8_31_32_33_default,
  json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0_default
]);
```
### rolldown
```js

//#region <data:application/json,"%31%32%33">
var json___31_32_33__default = "123";
//#endregion

//#region <data:application/json;base64,eyJ3b3JrcyI6dHJ1ZX0=>
var works$1 = true;
var json_base64_eyJ3b3JrcyI6dHJ1ZX0__default = { works: works$1 };
//#endregion

//#region <data:application/json;charset=UTF-8,%31%32%33>
var json_charset_UTF_8__31_32_33_default = 123;
//#endregion

//#region <data:application/json;charset=UTF-8;base64,eyJ3b3JrcyI6dHJ1ZX0=>
var works = true;
var json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0__default = { works };
//#endregion

//#region entry.js
console.log([
	json___31_32_33__default,
	json_base64_eyJ3b3JrcyI6dHJ1ZX0__default,
	json_charset_UTF_8__31_32_33_default,
	json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0__default
]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,11 @@
-var json_31_32_33_default = "123";
-var json_base64_eyJ3b3JrcyI6dHJ1ZX0_default = {
-    works: true
+var json___31_32_33__default = "123";
+var works$1 = true;
+var json_base64_eyJ3b3JrcyI6dHJ1ZX0__default = {
+    works: works$1
 };
-var json_charset_UTF_8_31_32_33_default = 123;
-var json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0_default = {
-    works: true
+var json_charset_UTF_8__31_32_33_default = 123;
+var works = true;
+var json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0__default = {
+    works
 };
-console.log([json_31_32_33_default, json_base64_eyJ3b3JrcyI6dHJ1ZX0_default, json_charset_UTF_8_31_32_33_default, json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0_default]);
+console.log([json___31_32_33__default, json_base64_eyJ3b3JrcyI6dHJ1ZX0__default, json_charset_UTF_8__31_32_33_default, json_charset_UTF_8_base64_eyJ3b3JrcyI6dHJ1ZX0__default]);

```