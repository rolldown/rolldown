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

//#region entry.js
let sideEffects = console.log("side effects");
let collide = 123;
console.log(obj.prop);
console.log(obj.defined);
console.log(injectedAndDefined);
console.log(injected.and.defined);
console.log(chain.prop.test);
console.log(chain2.prop2.test);
console.log(collide);
console.log(re_export);
console.log(reexpo.rt);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,22 +1,11 @@
-var obj2 = {};
-var sideEffects2 = console.log("this should be renamed");
-console.log("This is unused but still has side effects");
-var replace2 = {
-    test() {}
-};
-var replaceDot = {
-    test() {}
-};
-import {re_export as re_export2} from "external-pkg";
-import {undefined as reexpo_rt} from "external-pkg2";
-let sideEffects = console.log("side effects");
-let collide = 123;
-console.log(obj2.prop);
-console.log("defined");
-console.log("should be used");
-console.log("should be used");
-console.log(replace2.test);
-console.log(replaceDot.test);
+var sideEffects = console.log("side effects");
+var collide = 123;
+console.log(obj.prop);
+console.log(obj.defined);
+console.log(injectedAndDefined);
+console.log(injected.and.defined);
+console.log(chain.prop.test);
+console.log(chain2.prop2.test);
 console.log(collide);
-console.log(re_export2);
-console.log(reexpo_rt);
+console.log(re_export);
+console.log(reexpo.rt);

```