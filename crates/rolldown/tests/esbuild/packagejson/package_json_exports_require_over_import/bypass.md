# Reason
1. different fs
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

//#region node_modules/pkg/require.js
var require_require = __commonJS({ "node_modules/pkg/require.js"() {
	console.log("SUCCESS");
} });

//#endregion
//#region src/entry.js
require_require();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 var require_require = __commonJS({
-    "Users/user/project/node_modules/pkg/require.js"() {
+    "node_modules/pkg/require.js"() {
         console.log("SUCCESS");
     }
 });
 require_require();

```