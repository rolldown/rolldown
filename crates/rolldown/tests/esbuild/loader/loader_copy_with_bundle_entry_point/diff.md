# Reason
1. not support copy loader
# Diff
## /out/assets/some.file
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
--- esbuild	/out/assets/some.file
+++ rolldown	
@@ -1,1 +0,0 @@
-stuff;

```
## /out/src/entry.js
### esbuild
```js
// Users/user/project/src/entry.js
import x from "../assets/some.file";
console.log(x);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/src/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import x from "../assets/some.file";
-console.log(x);

```
## /out/src/entry.css
### esbuild
```js
/* Users/user/project/src/entry.css */
body {
  background: url("../assets/some.file");
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
-  background: url("../assets/some.file");
-}
\ No newline at end of file

```