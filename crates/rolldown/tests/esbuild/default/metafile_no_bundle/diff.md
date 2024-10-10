# Diff
## /out/entry.js
### esbuild
```js
import a from "pkg";
import b from "./file";
console.log(
  a,
  b,
  require("pkg2"),
  require("./file2"),
  import("./dynamic")
);
let exported;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-import a from "pkg";
-import b from "./file";
-console.log(a, b, require("pkg2"), require("./file2"), import("./dynamic"));
-let exported;

```