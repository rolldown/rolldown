# Reason
1. not support legal comments
# Diff
## /out/entry.css
### esbuild
```js
/* a.css */
a {
  zoom: 2;
}

/* b.css */
b {
  zoom: 2;
}

/* c.css */
c {
  zoom: 2;
}

/* entry.css */
/*! Copyright notice 1 */
/*! Copyright notice 2 */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,18 +0,0 @@
-/* a.css */
-a {
-  zoom: 2;
-}
-
-/* b.css */
-b {
-  zoom: 2;
-}
-
-/* c.css */
-c {
-  zoom: 2;
-}
-
-/* entry.css */
-/*! Copyright notice 1 */
-/*! Copyright notice 2 */
\ No newline at end of file

```