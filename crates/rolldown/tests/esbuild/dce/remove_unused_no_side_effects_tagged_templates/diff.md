<<<<<<< HEAD
# Diff
## /out.js
### esbuild
```js
// entry.js
// @__NO_SIDE_EFFECTS__
function foo() {
}
use(foo`keep`);
keep, alsoKeep;
`${keep}${alsoKeep}`;
```
### rolldown
```js

//#region entry.js
// @__NO_SIDE_EFFECTS__
function foo() {}
foo`remove`;
foo`remove${null}`;
foo`remove${123}`;
use(foo`keep`);
foo`remove this part ${keep} and this ${alsoKeep}`;
`remove this part ${keep} and this ${alsoKeep}`;

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.js
@@ -1,4 +1,7 @@
 function foo() {}
+foo`remove`;
+foo`remove${null}`;
+foo`remove${123}`;
 use(foo`keep`);
-(keep, alsoKeep);
-`${keep}${alsoKeep}`;
+foo`remove this part ${keep} and this ${alsoKeep}`;
+`remove this part ${keep} and this ${alsoKeep}`;

```