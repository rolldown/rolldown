# Reason
1. we should generate same output as esbuild in bundle mode
# Diff
## /out.js
### esbuild
```js
export { default as foo } from "./foo";
export { default as bar } from "./bar";
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
@@ -1,2 +1,3 @@
-export {default as foo} from "./foo";
-export {default as bar} from "./bar";
+import foo from "./foo";
+import bar from "./bar";
+export {bar, foo};

```