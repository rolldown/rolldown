# Reason
1. not support legal comments
# Diff
## /out/entry.css
### esbuild
```js
/* entry.css */
@media (x: y) {
  /**
   * @preserve
   */
  z {
    zoom: 2;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,9 +0,0 @@
-/* entry.css */
-@media (x: y) {
-  /**
-   * @preserve
-   */
-  z {
-    zoom: 2;
-  }
-}
\ No newline at end of file

```