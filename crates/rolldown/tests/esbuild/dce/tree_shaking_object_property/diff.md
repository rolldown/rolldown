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
let keep1 = { x };
let keep2 = { x };
let keep3 = { ...x };
let keep4 = { [x]: "x" };
let keep5 = { [x]() {} };
let keep6 = { get [x]() {} };
let keep7 = { set [x](_) {} };
let keep8 = { async [x]() {} };
let keep9 = { [{ toString() {} }]: "x" };

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,29 @@
-let keep1 = {
+var keep1 = {
     x
 };
-let keep2 = {
+var keep2 = {
     x
 };
-let keep3 = {
+var keep3 = {
     ...x
 };
-let keep4 = {
+var keep4 = {
     [x]: "x"
 };
-let keep5 = {
+var keep5 = {
     [x]() {}
 };
-let keep6 = {
+var keep6 = {
     get [x]() {}
 };
-let keep7 = {
+var keep7 = {
     set [x](_) {}
 };
-let keep8 = {
+var keep8 = {
     async [x]() {}
 };
-let keep9 = {
+var keep9 = {
     [{
         toString() {}
     }]: "x"
 };

```