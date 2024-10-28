# Diff
## /out/dot-define.js
### esbuild
```js
// dot-define.js
1 .c;
1 .c;
1["c"];
1["c"];
1();
1();
1 .c();
1 .c();
1["c"]();
1["c"]();
delete 1 .c;
delete 1 .c;
delete 1["c"];
delete 1["c"];
```
### rolldown
```js

//#region dot-define.js
1 .c;
(a?.b).c;
1["c"];
(a?.b)["c"];
1();
(a?.b)();
1 .c();
(a?.b).c();
1["c"]();
(a?.b)["c"]();
delete 1 .c;
delete (a?.b).c;
delete 1["c"];
delete (a?.b)["c"];

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/dot-define.js
+++ rolldown	dot-define.js
@@ -1,14 +1,14 @@
 (1).c;
-(1).c;
+a?.b.c;
 (1)["c"];
-(1)["c"];
+a?.b["c"];
 (1)();
-(1)();
+a?.b();
 (1).c();
-(1).c();
+a?.b.c();
 (1)["c"]();
-(1)["c"]();
+a?.b["c"]();
 delete (1).c;
-delete (1).c;
+delete a?.b.c;
 delete (1)["c"];
-delete (1)["c"];
+delete a?.b["c"];

```