# Reason
1. Rolldown generate same output as esbuild in bundle mode
# Diff
## /out.js
### esbuild
```js
function earlyReturn() {
  onlyWithKeep();
}
function loop() {
  if (foo()) {
    bar();
    return;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +0,0 @@
-function earlyReturn() {
-    onlyWithKeep();
-}
-function loop() {
-    if (foo()) {
-        bar();
-        return;
-    }
-}

```