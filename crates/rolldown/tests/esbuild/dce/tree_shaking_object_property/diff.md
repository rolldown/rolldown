# Diff
## /out.js
### esbuild
```js
let keep1 = { x };
let keep2 = { x };
let keep3 = { ...x };
let keep4 = { [x]: "x" };
let keep5 = { [x]() {
} };
let keep6 = { get [x]() {
} };
let keep7 = { set [x](_) {
} };
let keep8 = { async [x]() {
} };
let keep9 = { [{ toString() {
} }]: "x" };
```
### rolldown
```js
//#region entry.js
({ x });
({ x });
({ ...x });
({ [x]: "x" });
({ [x]() {} });
({ get [x]() {} });
({ set [x](_) {} });
({ async [x]() {} });

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,24 @@
-let keep1 = {
+({
     x
-};
-let keep2 = {
+});
+({
     x
-};
-let keep3 = {
+});
+({
     ...x
-};
-let keep4 = {
+});
+({
     [x]: "x"
-};
-let keep5 = {
+});
+({
     [x]() {}
-};
-let keep6 = {
+});
+({
     get [x]() {}
-};
-let keep7 = {
+});
+({
     set [x](_) {}
-};
-let keep8 = {
+});
+({
     async [x]() {}
-};
-let keep9 = {
-    [{
-        toString() {}
-    }]: "x"
-};
+});

```