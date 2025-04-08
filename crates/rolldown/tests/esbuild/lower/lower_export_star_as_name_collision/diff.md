# Reason
1. should not transform `export * as ns from 'mod'` above es2019
# Diff
## /out.js
### esbuild
```js
// nested.js
import * as foo from "path2";
var foo2 = 123;

// entry.js
import * as ns from "path1";
console.log(foo, foo2);
var ns2 = 123;
export {
  ns,
  ns2 as sn
};
```
### rolldown
```js
import * as foo from "path2";
import * as ns$1 from "path1";

//#region nested.js
let foo$1 = 123;

//#endregion
//#region entry.js
console.log(foo, foo$1);
let ns$2 = 123;

//#endregion
export { ns$1 as ns, ns$2 as sn };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 import * as foo from "path2";
-var foo2 = 123;
-import * as ns from "path1";
-console.log(foo, foo2);
-var ns2 = 123;
-export {ns, ns2 as sn};
+import * as ns$1 from "path1";
+var foo$1 = 123;
+console.log(foo, foo$1);
+var ns$2 = 123;
+export {ns$1 as ns, ns$2 as sn};

```