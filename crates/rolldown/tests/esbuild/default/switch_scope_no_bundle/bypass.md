# Reason
1. different deconflict naming style
# Diff
## /out.js
### esbuild
```js
switch (foo) {
  default:
    var foo;
}
switch (bar) {
  default:
    let a;
}
```
### rolldown
```js

//#region entry.js
switch (foo) {
	default: var foo;
}
switch (bar) {
	default: let bar$1;
}

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -3,6 +3,6 @@
         var foo;
 }
 switch (bar) {
     default:
-        let a;
+        let bar$1;
 }

```