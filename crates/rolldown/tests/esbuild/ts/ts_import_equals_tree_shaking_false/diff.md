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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {foo} from "pkg";
-const used = foo.used;
-export {used};

```