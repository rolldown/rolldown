# Diff
## /out/some-BYATPJRB.file
### esbuild
```js
stuff
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/some-BYATPJRB.file
+++ rolldown	
@@ -1,1 +0,0 @@
-stuff;

```
## /out/src/entry.css
### esbuild
```js
/* Users/user/project/src/entry.css */
body {
  background: url("../some-BYATPJRB.file");
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/src/entry.css
+++ rolldown	
@@ -1,4 +0,0 @@
-/* Users/user/project/src/entry.css */
-body {
-  background: url("../some-BYATPJRB.file");
-}
\ No newline at end of file

```