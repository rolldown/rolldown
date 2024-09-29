## /out.js
### esbuild
```js
export * from "foo";
```
### rolldown
```js
import "foo";

export * from "foo"


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,1 +1,2 @@
+import "foo";
 export * from "foo";

```
