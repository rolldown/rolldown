# Reason
1. should have same output in bundle mode, https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.js%00%27use+strict%27%0A%09%09%09%09%27use+loose%27%0A%09%09%09%09a%0A%09%09%09%09b%0A&b=%00file.js%00&o=%7B%0A++treeShaking%3A+true%2C%0A++external%3A+%5B%22c%22%2C+%22a%22%2C+%22b%22%5D%2C%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22esm%22%0A%7D
# Diff
## /out.js
### esbuild
```js
"use strict";"use loose";a,b;
```
### rolldown
```js

//#region entry.js
"use loose";
a;
b;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-"use strict";
 "use loose";
-(a, b);
+a;
+b;

```