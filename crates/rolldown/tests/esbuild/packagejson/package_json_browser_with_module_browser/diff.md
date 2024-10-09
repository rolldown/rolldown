# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.browser.esm.js
function main_browser_esm_default() {
  return 123;
}

// Users/user/project/src/entry.js
console.log(main_browser_esm_default());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function main_browser_esm_default() {
-    return 123;
-}
-console.log(main_browser_esm_default());

```