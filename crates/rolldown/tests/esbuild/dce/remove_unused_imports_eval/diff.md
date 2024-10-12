# Diff
## /out.js
### esbuild
```js
import a from "a";
import * as b from "b";
import { c } from "c";
eval("foo(a, b, c)");
```
### rolldown
```js
import { default as a } from "a";
import * as b from "b";
import { c } from "c";

//#region entry.js
eval("foo(a, b, c)");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-import a from "a";
+import {default as a} from "a";
 import * as b from "b";
 import {c} from "c";
 eval("foo(a, b, c)");

```