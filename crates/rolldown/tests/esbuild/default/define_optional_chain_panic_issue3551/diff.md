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

```
### diff
```diff
===================================================================
--- esbuild	/out/id-define.js
+++ rolldown	
@@ -1,14 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/dot-define.js
+++ rolldown	
@@ -1,14 +0,0 @@
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

```