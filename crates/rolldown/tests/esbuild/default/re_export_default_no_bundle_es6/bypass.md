# Reason
1. trivial diff
# Diff
## /out.js
### esbuild
```js
import { default as default2 } from "./foo";
import { default as default3 } from "./bar";
export {
  default3 as bar,
  default2 as foo
};
```
### rolldown
```js
import foo from "./foo";
import bar from "./bar";

export { bar, foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-import {default as default2} from "./foo";
-import {default as default3} from "./bar";
-export {default3 as bar, default2 as foo};
+import foo from "./foo";
+import bar from "./bar";
+export {bar, foo};

```