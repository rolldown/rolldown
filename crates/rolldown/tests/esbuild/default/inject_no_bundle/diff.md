# Reason
1. generate wrong syntax when Exported is `StringLiteral`, and rest part of esbuild gen is weird since there is no need to rename
# Diff
## /out.js
### esbuild
```js
var obj2 = {};
var sideEffects2 = console.log("this should be renamed");
console.log("This is unused but still has side effects");
var replace2 = {
  test() {
  }
};
var replaceDot = {
  test() {
  }
};
import { re_export as re_export2 } from "external-pkg";
import { "reexpo.rt" as reexpo_rt } from "external-pkg2";
let sideEffects = console.log("side effects");
let collide = 123;
console.log(obj2.prop);
console.log("defined");
console.log("should be used");
console.log("should be used");
console.log(replace2.test);
console.log(replaceDot.test);
console.log(collide);
console.log(re_export2);
console.log(reexpo_rt);
```
### rolldown
```js
import { re_export as re_export$1 } from "external-pkg";
import { "reexpo.rt" as reexpo_rt } from "external-pkg2";

//#region replacement.js
let replace$1 = { test() {} };
let replaceDot = { test() {} };

//#endregion
//#region inject.js
let obj$1 = {};
let sideEffects$1 = console.log("this should be renamed");

//#endregion
//#region entry.js
let sideEffects = console.log("side effects");
let collide = 123;
console.log(obj$1.prop);
console.log("defined");
console.log("should be used");
console.log("should be used");
console.log(replace$1.test);
console.log(replaceDot.test);
console.log(collide);
console.log(re_export$1);
console.log(reexpo_rt);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,22 +1,21 @@
-var obj2 = {};
-var sideEffects2 = console.log("this should be renamed");
-console.log("This is unused but still has side effects");
-var replace2 = {
+import {re_export as re_export$1} from "external-pkg";
+import {undefined as reexpo_rt} from "external-pkg2";
+var replace$1 = {
     test() {}
 };
 var replaceDot = {
     test() {}
 };
-import {re_export as re_export2} from "external-pkg";
-import {undefined as reexpo_rt} from "external-pkg2";
-let sideEffects = console.log("side effects");
-let collide = 123;
-console.log(obj2.prop);
+var obj$1 = {};
+var sideEffects$1 = console.log("this should be renamed");
+var sideEffects = console.log("side effects");
+var collide = 123;
+console.log(obj$1.prop);
 console.log("defined");
 console.log("should be used");
 console.log("should be used");
-console.log(replace2.test);
+console.log(replace$1.test);
 console.log(replaceDot.test);
 console.log(collide);
-console.log(re_export2);
+console.log(re_export$1);
 console.log(reexpo_rt);

```