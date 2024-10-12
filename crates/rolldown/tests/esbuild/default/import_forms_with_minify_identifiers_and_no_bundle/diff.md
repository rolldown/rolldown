# Diff
## /out.js
### esbuild
```js
import "foo";
import {} from "foo";
import * as o from "foo";
import { a as r, b as m } from "foo";
import t from "foo";
import f, * as i from "foo";
import p, { a2 as s, b as n } from "foo";
const a = [
  import("foo"),
  function() {
    return import("foo");
  }
];
console.log(o, r, m, t, f, i, p, s, n, a);
```
### rolldown
```js
import * as ns2 from "foo";
import * as ns from "foo";
import { a, a2, b as c, b as c3, default as def, default as def2, default as def3 } from "foo";

//#region entry.js
const imp = [import("foo"), function() {
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
-import * as o from "foo";
-import {a as r, b as m} from "foo";
-import t from "foo";
-import f, * as i from "foo";
-import p, {a2 as s, b as n} from "foo";
-const a = [import("foo"), function () {
+import * as ns2 from "foo";
+import * as ns from "foo";
+import {a, a2, b as c, b as c3, default as def, default as def2, default as def3} from "foo";
+var imp = [import("foo"), function () {
     return import("foo");
 }];
-console.log(o, r, m, t, f, i, p, s, n, a);
+console.log(ns, a, c, def, def2, ns2, def3, a2, c3, imp);

```