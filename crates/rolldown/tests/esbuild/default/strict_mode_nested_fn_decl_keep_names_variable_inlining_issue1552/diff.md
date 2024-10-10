# Diff
## /out/entry.js
### esbuild
```js
export function outer() {
  {
    let inner = function() {
      return Math.random();
    };
    __name(inner, "inner");
    const x = inner();
    console.log(x);
  }
}
__name(outer, "outer"), outer();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,11 +0,0 @@
-export function outer() {
-    {
-        let inner = function () {
-            return Math.random();
-        };
-        __name(inner, "inner");
-        const x = inner();
-        console.log(x);
-    }
-}
-(__name(outer, "outer"), outer());

```