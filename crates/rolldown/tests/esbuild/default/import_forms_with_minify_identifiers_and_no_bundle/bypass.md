# Reason
1. Could be done in minifier
2. Rolldown try to merge external default import binding
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
import * as ns from "foo";
import def, { a, a2, b } from "foo";

//#region entry.js
const imp = [import("foo"), function() {
	return import("foo");
}];
console.log(ns, a, b, def, def, ns, def, a2, b, imp);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,6 @@
-import "foo";
-import "foo";
-import * as o from "foo";
-import {a as r, b as m} from "foo";
-import t from "foo";
-import f, * as i from "foo";
-import p, {a2 as s, b as n} from "foo";
-const a = [import("foo"), function () {
+import * as ns from "foo";
+import def, {a, a2, b} from "foo";
+var imp = [import("foo"), function () {
     return import("foo");
 }];
-console.log(o, r, m, t, f, i, p, s, n, a);
+console.log(ns, a, b, def, def, ns, def, a2, b, imp);

```