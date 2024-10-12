# Diff
## /out.js
### esbuild
```js
// shouldUsePercent_1.txt
var shouldUsePercent_1_default = "data:text/plain;charset=utf-8,%0A%0A%0A";

// shouldUsePercent_2.txt
var shouldUsePercent_2_default = "data:text/plain;charset=utf-8,%0A%0A%0A%0A";

// shouldUseBase64_1.txt
var shouldUseBase64_1_default = "data:text/plain;charset=utf-8;base64,CgoKCgo=";

// shouldUseBase64_2.txt
var shouldUseBase64_2_default = "data:text/plain;charset=utf-8;base64,CgoKCgoK";

// entry.js
console.log(
  shouldUsePercent_1_default,
  shouldUsePercent_2_default,
  shouldUseBase64_1_default,
  shouldUseBase64_2_default
);
```
### rolldown
```js

//#region shouldUsePercent_1.txt
var shouldUsePercent_1_default = "data:text/plain;charset=utf-8,%0A%0A%0A";

//#endregion
//#region shouldUsePercent_2.txt
var shouldUsePercent_2_default = "data:text/plain;charset=utf-8,%0A%0A%0A%0A";

//#endregion
//#region shouldUseBase64_1.txt
var shouldUseBase64_1_default = "data:text/plain;base64,CgoKCgo=";

//#endregion
//#region shouldUseBase64_2.txt
var shouldUseBase64_2_default = "data:text/plain;base64,CgoKCgoK";

//#endregion
//#region entry.js
console.log(shouldUsePercent_1_default, shouldUsePercent_2_default, shouldUseBase64_1_default, shouldUseBase64_2_default);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
 var shouldUsePercent_1_default = "data:text/plain;charset=utf-8,%0A%0A%0A";
 var shouldUsePercent_2_default = "data:text/plain;charset=utf-8,%0A%0A%0A%0A";
-var shouldUseBase64_1_default = "data:text/plain;charset=utf-8;base64,CgoKCgo=";
-var shouldUseBase64_2_default = "data:text/plain;charset=utf-8;base64,CgoKCgoK";
+var shouldUseBase64_1_default = "data:text/plain;base64,CgoKCgo=";
+var shouldUseBase64_2_default = "data:text/plain;base64,CgoKCgoK";
 console.log(shouldUsePercent_1_default, shouldUsePercent_2_default, shouldUseBase64_1_default, shouldUseBase64_2_default);

```