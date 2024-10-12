# Diff
## /out/id-define.js
### esbuild
```js
// id-define.js
1?.y.z;
(1?.y).z;
1?.y["z"];
(1?.y)["z"];
1?.y();
(1?.y)();
1?.y.z();
(1?.y).z();
1?.y["z"]();
(1?.y)["z"]();
delete 1?.y.z;
delete (1?.y).z;
delete 1?.y["z"];
delete (1?.y)["z"];
```
### rolldown
```js

//#region id-define.js
x?.y.z;
x?.y.z;
x?.y["z"];
x?.y["z"];
x?.y();
x?.y();
x?.y.z();
x?.y.z();
x?.y["z"]();
x?.y["z"]();
delete x?.y.z;
delete x?.y.z;
delete x?.y["z"];
delete x?.y["z"];

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/id-define.js
+++ rolldown	id-define.js
@@ -1,14 +1,14 @@
-(1)?.y.z;
-(1)?.y.z;
-(1)?.y["z"];
-(1)?.y["z"];
-(1)?.y();
-(1)?.y();
-(1)?.y.z();
-(1)?.y.z();
-(1)?.y["z"]();
-(1)?.y["z"]();
-delete (1)?.y.z;
-delete (1)?.y.z;
-delete (1)?.y["z"];
-delete (1)?.y["z"];
+x?.y.z;
+x?.y.z;
+x?.y["z"];
+x?.y["z"];
+x?.y();
+x?.y();
+x?.y.z();
+x?.y.z();
+x?.y["z"]();
+x?.y["z"]();
+delete x?.y.z;
+delete x?.y.z;
+delete x?.y["z"];
+delete x?.y["z"];

```
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
a?.b.c;
a?.b.c;
a?.b["c"];
a?.b["c"];
a?.b();
a?.b();
a?.b.c();
a?.b.c();
a?.b["c"]();
a?.b["c"]();
delete a?.b.c;
delete a?.b.c;
delete a?.b["c"];
delete a?.b["c"];

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/dot-define.js
+++ rolldown	dot-define.js
@@ -1,14 +1,14 @@
-(1).c;
-(1).c;
-(1)["c"];
-(1)["c"];
-(1)();
-(1)();
-(1).c();
-(1).c();
-(1)["c"]();
-(1)["c"]();
-delete (1).c;
-delete (1).c;
-delete (1)["c"];
-delete (1)["c"];
+a?.b.c;
+a?.b.c;
+a?.b["c"];
+a?.b["c"];
+a?.b();
+a?.b();
+a?.b.c();
+a?.b.c();
+a?.b["c"]();
+a?.b["c"]();
+delete a?.b.c;
+delete a?.b.c;
+delete a?.b["c"];
+delete a?.b["c"];

```