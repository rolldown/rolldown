# Reason
1. generate wrong output when css as entry and has shared css
# Diff
## /out/entries/entry.css
### esbuild
```js
/* src/shared/common.css */
div {
  background: url("../common-LSAMBFUD.png");
}

/* src/entries/entry.css */
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out/entries/entry.css
+++ rolldown	entries_entry.css
@@ -1,6 +1,1 @@
-/* src/shared/common.css */
-div {
-  background: url("../common-LSAMBFUD.png");
-}
 
-/* src/entries/entry.css */
\ No newline at end of file

```
## /out/entries/other/entry.css
### esbuild
```js
/* src/shared/common.css */
div {
  background: url("../../common-LSAMBFUD.png");
}

/* src/entries/other/entry.css */
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entries/other/entry.css
+++ rolldown	
@@ -1,6 +0,0 @@
-/* src/shared/common.css */
-div {
-  background: url("../../common-LSAMBFUD.png");
-}
-
-/* src/entries/other/entry.css */
\ No newline at end of file

```