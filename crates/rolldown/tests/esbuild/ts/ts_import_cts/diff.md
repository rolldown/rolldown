# Diff
## /out.js
### esbuild
```js
// required.cjs
var require_required = __commonJS({
  "required.cjs"() {
    console.log("works");
  }
});

// entry.ts
require_required();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_required = __commonJS({
-    "required.cjs"() {
-        console.log("works");
-    }
-});
-require_required();

```