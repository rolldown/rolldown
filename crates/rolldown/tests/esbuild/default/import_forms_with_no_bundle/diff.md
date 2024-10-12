# Diff
## /out.js
### esbuild
```js
import "foo";
import {} from "foo";
import * as ns from "foo";
import { a, b as c } from "foo";
import def from "foo";
import def2, * as ns2 from "foo";
import def3, { a2, b as c3 } from "foo";
const imp = [
  import("foo"),
  function nested() {
    return import("foo");
  }
];
console.log(ns, a, c, def, def2, ns2, def3, a2, c3, imp);
```
### rolldown
```js
import * as ns2 from "foo";
import * as ns from "foo";
import { a, a2, b as c, b as c3, default as def, default as def2, default as def3 } from "foo";

//#region entry.js
const imp = [import("foo"), function nested() {
	return import("foo");
}];
console.log(ns, a, c, def, def2, ns2, def3, a2, c3, imp);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,7 @@
-import "foo";
-import "foo";
+import * as ns2 from "foo";
 import * as ns from "foo";
-import {a, b as c} from "foo";
-import def from "foo";
-import def2, * as ns2 from "foo";
-import def3, {a2, b as c3} from "foo";
-const imp = [import("foo"), function nested() {
+import {a, a2, b as c, b as c3, default as def, default as def2, default as def3} from "foo";
+var imp = [import("foo"), function nested() {
     return import("foo");
 }];
 console.log(ns, a, c, def, def2, ns2, def3, a2, c3, imp);

```