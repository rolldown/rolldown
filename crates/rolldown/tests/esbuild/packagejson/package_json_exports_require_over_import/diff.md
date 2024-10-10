# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/pkg/require.js
var require_require = __commonJS({
  "Users/user/project/node_modules/pkg/require.js"() {
    console.log("SUCCESS");
  }
});

// Users/user/project/src/entry.js
require_require();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_require = __commonJS({
-    "Users/user/project/node_modules/pkg/require.js"() {
-        console.log("SUCCESS");
-    }
-});
-require_require();

```