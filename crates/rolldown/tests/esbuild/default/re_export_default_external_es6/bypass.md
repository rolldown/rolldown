# Reason
1. different indirect reexport
# Diff
## /out.js
### esbuild
```js
// entry.js
import { default as default3 } from "foo";

// bar.js
import { default as default2 } from "bar";
export {
  default2 as bar,
  default3 as foo
};
```
### rolldown
```js
import foo from "foo";
import bar from "bar";

export { bar, foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-import {default as default3} from "foo";
-import {default as default2} from "bar";
-export {default2 as bar, default3 as foo};
+import foo from "foo";
+import bar from "bar";
+export {bar, foo};

```