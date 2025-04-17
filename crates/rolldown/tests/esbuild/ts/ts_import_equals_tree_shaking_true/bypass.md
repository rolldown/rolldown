# Reason
1. rolldown is not ts aware, it's not possible to support for now
2. sub optimal
# Diff
## /out.js
### esbuild
```js
import { foo } from "pkg";
const used = foo.used;
export { used };
```
### rolldown
```js
import { foo } from "pkg";

//#region entry.ts
var used = foo.used;

export { used };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
 import {foo} from "pkg";
-const used = foo.used;
+var used = foo.used;
 export {used};

```