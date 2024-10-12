# Diff
## entry.css
### esbuild
```js
/* what */
.foo {
  color: red;
}

/* entry.css */
```
### rolldown
```js
@import './what';

```
### diff
```diff
===================================================================
--- esbuild	entry.css
+++ rolldown	entry.css
@@ -1,6 +1,1 @@
-/* what */
-.foo {
-  color: red;
-}
-
-/* entry.css */
\ No newline at end of file
+@import './what';

```