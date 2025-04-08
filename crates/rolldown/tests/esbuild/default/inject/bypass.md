# Reason
1. the last line diff is because different inject implementation between `Oxc inject`(follows rollup inject) and `esbuild`
# Diff
## /out.js
### esbuild
```js
// inject.js
var obj = {};
var sideEffects = console.log("side effects");

// node_modules/unused/index.js
console.log("This is unused but still has side effects");

// replacement.js
var replace = {
  test() {
  }
};
var replace2 = {
  test() {
  }
};

// re-export.js
var import_external_pkg = require("external-pkg");
var import_external_pkg2 = require("external-pkg2");

// entry.js
var sideEffects2 = console.log("this should be renamed");
var collide = 123;
console.log(obj.prop);
console.log("defined");
console.log("should be used");
console.log("should be used");
console.log(replace.test);
console.log(replace2.test);
console.log(collide);
console.log(import_external_pkg.re_export);
console.log(re_export2);
```
### rolldown
```js
"use strict";

const external_pkg = __toESM(require("external-pkg"));
const external_pkg2 = __toESM(require("external-pkg2"));

//#region replacement.js
let replace = { test() {} };
let $inject_chain2_prop2 = { test() {} };

//#endregion
//#region inject.js
let obj = {};
let sideEffects$1 = console.log("side effects");

//#endregion
//#region entry.js
let sideEffects = console.log("this should be renamed");
let collide = 123;
console.log(obj.prop);
console.log("defined");
console.log("should be used");
console.log("should be used");
console.log(replace.test);
console.log($inject_chain2_prop2.test);
console.log(collide);
console.log(external_pkg.re_export);
console.log(external_pkg2.re.export);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,22 +1,21 @@
-var obj = {};
-var sideEffects = console.log("side effects");
-console.log("This is unused but still has side effects");
+var external_pkg = __toESM(require("external-pkg"));
+var external_pkg2 = __toESM(require("external-pkg2"));
 var replace = {
     test() {}
 };
-var replace2 = {
+var $inject_chain2_prop2 = {
     test() {}
 };
-var import_external_pkg = require("external-pkg");
-var import_external_pkg2 = require("external-pkg2");
-var sideEffects2 = console.log("this should be renamed");
+var obj = {};
+var sideEffects$1 = console.log("side effects");
+var sideEffects = console.log("this should be renamed");
 var collide = 123;
 console.log(obj.prop);
 console.log("defined");
 console.log("should be used");
 console.log("should be used");
 console.log(replace.test);
-console.log(replace2.test);
+console.log($inject_chain2_prop2.test);
 console.log(collide);
-console.log(import_external_pkg.re_export);
-console.log(re_export2);
+console.log(external_pkg.re_export);
+console.log(external_pkg2.re.export);

```