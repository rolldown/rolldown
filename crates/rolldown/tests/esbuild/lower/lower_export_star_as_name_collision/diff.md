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
import * as _foo from "path2";
import * as _ns from "path1";

//#region nested.js
let foo = 123;

//#endregion
//#region entry.js
console.log(_foo, foo);
let ns = 123;

//#endregion
export { _ns as ns, ns as sn };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
-import * as foo from "path2";
-var foo2 = 123;
-import * as ns from "path1";
-console.log(foo, foo2);
-var ns2 = 123;
-export {ns, ns2 as sn};
+import * as _foo from "path2";
+import * as _ns from "path1";
+var foo = 123;
+console.log(_foo, foo);
+var ns = 123;
+export {_ns as ns, ns as sn};

```