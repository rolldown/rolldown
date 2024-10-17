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
/*! Copyright notice 1 */

/* b.css */
b {
  zoom: 2;
}
/*! Copyright notice 1 */

/* c.css */
c {
  zoom: 2;
}
/*! Copyright notice 2 */

/* entry.css */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,19 +0,0 @@
-/* a.css */
-a {
-  zoom: 2;
-}
-/*! Copyright notice 1 */
-
-/* b.css */
-b {
-  zoom: 2;
-}
-/*! Copyright notice 1 */
-
-/* c.css */
-c {
-  zoom: 2;
-}
-/*! Copyright notice 2 */
-
-/* entry.css */
\ No newline at end of file

```