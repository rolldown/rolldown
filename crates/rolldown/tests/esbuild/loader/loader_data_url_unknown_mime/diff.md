# Reason
1. should not transform `ImoprtDefaultSpecifier` as `import {default as x}`
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import a from "data:some/thing;what,someData%31%32%33";
import b from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";
console.log(a, b);
```
### rolldown
```js
import { default as a } from "data:some/thing;what,someData%31%32%33";
import { default as b } from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";

//#region entry.js
console.log(a, b);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-import a from "data:some/thing;what,someData%31%32%33";
-import b from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";
+import {default as a} from "data:some/thing;what,someData%31%32%33";
+import {default as b} from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";
 console.log(a, b);

```