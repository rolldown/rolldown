# Diff
## /out/supported.js
### esbuild
```js
// supported.ts
console.log(
  // a number or string literal,
  123 /* X0 */,
  "x" /* X1 */,
  // a unary +, -, or ~ applied to a numeric constant expression,
  1 /* X2 */,
  -2 /* X3 */,
  -4 /* X4 */,
  // a binary +, -, *, /, %, **, <<, >>, >>>, |, &, ^ applied to two numeric constant expressions,
  3 /* X5 */,
  -1 /* X6 */,
  6 /* X7 */,
  0.5 /* X8 */,
  1 /* X9 */,
  8 /* X10 */,
  4 /* X11 */,
  -5 /* X12 */,
  2147483643 /* X13 */,
  13 /* X14 */,
  4 /* X15 */,
  9 /* X16 */,
  // a template expression where each substitution expression is a constant expression,
  "x0" /* X17 */,
  "0x" /* X18 */,
  "xy" /* X19 */,
  "NaN" /* X20 */,
  "Infinity" /* X21 */,
  "-Infinity" /* X22 */,
  "0" /* X23 */,
  // a template expression where each substitution expression is a constant expression,
  "A0BxC-31246D" /* X24 */,
  // a parenthesized constant expression,
  321 /* X25 */,
  // a dotted name that references an enum member with an enum literal type, or
  123 /* X26 */,
  "123x" /* X27 */,
  "x123" /* X28 */,
  "a123b" /* X29 */,
  123 /* X30 */,
  "123x" /* X31 */,
  "x123" /* X32 */,
  "a123b" /* X33 */,
  // a dotted name indexed by a string literal (e.g. x.y["z"]) that references an enum member with an enum literal type."
  "x" /* X34 */,
  "xy" /* X35 */,
  "yx" /* X36 */,
  "axb" /* X37 */,
  "x" /* X38 */,
  "xy" /* X39 */,
  "yx" /* X40 */,
  "axb" /* X41 */
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/supported.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123, "x", 1, -2, -4, 3, -1, 6, 0.5, 1, 8, 4, -5, 2147483643, 13, 4, 9, "x0", "0x", "xy", "NaN", "Infinity", "-Infinity", "0", "A0BxC-31246D", 321, 123, "123x", "x123", "a123b", 123, "123x", "x123", "a123b", "x", "xy", "yx", "axb", "x", "xy", "yx", "axb");

```
## /out/not-supported.js
### esbuild
```js
// not-supported.ts
var NonIntegerNumberToString = ((NonIntegerNumberToString2) => {
  NonIntegerNumberToString2["SUPPORTED"] = "1";
  NonIntegerNumberToString2["UNSUPPORTED"] = "" + 1.5;
  return NonIntegerNumberToString2;
})(NonIntegerNumberToString || {});
console.log(
  "1" /* SUPPORTED */,
  NonIntegerNumberToString.UNSUPPORTED
);
var OutOfBoundsNumberToString = ((OutOfBoundsNumberToString2) => {
  OutOfBoundsNumberToString2["SUPPORTED"] = "1000000000";
  OutOfBoundsNumberToString2["UNSUPPORTED"] = "" + 1e12;
  return OutOfBoundsNumberToString2;
})(OutOfBoundsNumberToString || {});
console.log(
  "1000000000" /* SUPPORTED */,
  OutOfBoundsNumberToString.UNSUPPORTED
);
console.log(
  "null" /* NULL */,
  "true" /* TRUE */,
  "false" /* FALSE */,
  "123" /* BIGINT */
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/not-supported.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var NonIntegerNumberToString = (NonIntegerNumberToString2 => {
-    NonIntegerNumberToString2["SUPPORTED"] = "1";
-    NonIntegerNumberToString2["UNSUPPORTED"] = "" + 1.5;
-    return NonIntegerNumberToString2;
-})(NonIntegerNumberToString || ({}));
-console.log("1", NonIntegerNumberToString.UNSUPPORTED);
-var OutOfBoundsNumberToString = (OutOfBoundsNumberToString2 => {
-    OutOfBoundsNumberToString2["SUPPORTED"] = "1000000000";
-    OutOfBoundsNumberToString2["UNSUPPORTED"] = "" + 1e12;
-    return OutOfBoundsNumberToString2;
-})(OutOfBoundsNumberToString || ({}));
-console.log("1000000000", OutOfBoundsNumberToString.UNSUPPORTED);
-console.log("null", "true", "false", "123");

```