# Reason
1. not support const enum inline
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

//#region supported.ts
var Foo = /* @__PURE__ */ function(Foo$1) {
	Foo$1[Foo$1["X0"] = 123] = "X0";
	Foo$1["X1"] = "x";
	Foo$1[Foo$1["X2"] = 1] = "X2";
	Foo$1[Foo$1["X3"] = -2] = "X3";
	Foo$1[Foo$1["X4"] = -4] = "X4";
	Foo$1[Foo$1["X5"] = 3] = "X5";
	Foo$1[Foo$1["X6"] = -1] = "X6";
	Foo$1[Foo$1["X7"] = 6] = "X7";
	Foo$1[Foo$1["X8"] = .5] = "X8";
	Foo$1[Foo$1["X9"] = 1] = "X9";
	Foo$1[Foo$1["X10"] = 8] = "X10";
	Foo$1[Foo$1["X11"] = 4] = "X11";
	Foo$1[Foo$1["X12"] = -5] = "X12";
	Foo$1[Foo$1["X13"] = 2147483643] = "X13";
	Foo$1[Foo$1["X14"] = 13] = "X14";
	Foo$1[Foo$1["X15"] = 4] = "X15";
	Foo$1[Foo$1["X16"] = 9] = "X16";
	Foo$1["X17"] = "x0";
	Foo$1["X18"] = "0x";
	Foo$1["X19"] = "xy";
	Foo$1["X20"] = "NaN";
	Foo$1["X21"] = "Infinity";
	Foo$1["X22"] = "-Infinity";
	Foo$1["X23"] = "0";
	Foo$1["X24"] = "ABCD";
	Foo$1[Foo$1["X25"] = 321] = "X25";
	Foo$1[Foo$1["X26"] = 123] = "X26";
	Foo$1["X27"] = "123x";
	Foo$1["X28"] = "x123";
	Foo$1["X29"] = "ab";
	Foo$1[Foo$1["X30"] = Foo$1.X0] = "X30";
	Foo$1[Foo$1["X31"] = Foo$1.X0 + "x"] = "X31";
	Foo$1[Foo$1["X32"] = "x" + Foo$1.X0] = "X32";
	Foo$1["X33"] = "ab";
	Foo$1["X34"] = "x";
	Foo$1["X35"] = "xy";
	Foo$1["X36"] = "yx";
	Foo$1["X37"] = "ab";
	Foo$1[Foo$1["X38"] = Foo$1["X1"]] = "X38";
	Foo$1[Foo$1["X39"] = Foo$1["X1"] + "y"] = "X39";
	Foo$1[Foo$1["X40"] = "y" + Foo$1["X1"]] = "X40";
	Foo$1["X41"] = "ab";
	return Foo$1;
}(Foo || {});
console.log(
	// a number or string literal,
	Foo.X0,
	Foo.X1,
	// a unary +, -, or ~ applied to a numeric constant expression,
	Foo.X2,
	Foo.X3,
	Foo.X4,
	// a binary +, -, *, /, %, **, <<, >>, >>>, |, &, ^ applied to two numeric constant expressions,
	Foo.X5,
	Foo.X6,
	Foo.X7,
	Foo.X8,
	Foo.X9,
	Foo.X10,
	Foo.X11,
	Foo.X12,
	Foo.X13,
	Foo.X14,
	Foo.X15,
	Foo.X16,
	// a template expression where each substitution expression is a constant expression,
	Foo.X17,
	Foo.X18,
	Foo.X19,
	Foo.X20,
	Foo.X21,
	Foo.X22,
	Foo.X23,
	// a template expression where each substitution expression is a constant expression,
	Foo.X24,
	// a parenthesized constant expression,
	Foo.X25,
	// a dotted name that references an enum member with an enum literal type, or
	Foo.X26,
	Foo.X27,
	Foo.X28,
	Foo.X29,
	Foo.X30,
	Foo.X31,
	Foo.X32,
	Foo.X33,
	// a dotted name indexed by a string literal (e.g. x.y["z"]) that references an enum member with an enum literal type."
	Foo.X34,
	Foo.X35,
	Foo.X36,
	Foo.X37,
	Foo.X38,
	Foo.X39,
	Foo.X40,
	Foo.X41
);

```
### diff
```diff
===================================================================
--- esbuild	/out/supported.js
+++ rolldown	supported.js
@@ -1,1 +1,46 @@
-console.log(123, "x", 1, -2, -4, 3, -1, 6, 0.5, 1, 8, 4, -5, 2147483643, 13, 4, 9, "x0", "0x", "xy", "NaN", "Infinity", "-Infinity", "0", "A0BxC-31246D", 321, 123, "123x", "x123", "a123b", 123, "123x", "x123", "a123b", "x", "xy", "yx", "axb", "x", "xy", "yx", "axb");
+var Foo = (function (Foo$1) {
+    Foo$1[Foo$1["X0"] = 123] = "X0";
+    Foo$1["X1"] = "x";
+    Foo$1[Foo$1["X2"] = 1] = "X2";
+    Foo$1[Foo$1["X3"] = -2] = "X3";
+    Foo$1[Foo$1["X4"] = -4] = "X4";
+    Foo$1[Foo$1["X5"] = 3] = "X5";
+    Foo$1[Foo$1["X6"] = -1] = "X6";
+    Foo$1[Foo$1["X7"] = 6] = "X7";
+    Foo$1[Foo$1["X8"] = .5] = "X8";
+    Foo$1[Foo$1["X9"] = 1] = "X9";
+    Foo$1[Foo$1["X10"] = 8] = "X10";
+    Foo$1[Foo$1["X11"] = 4] = "X11";
+    Foo$1[Foo$1["X12"] = -5] = "X12";
+    Foo$1[Foo$1["X13"] = 2147483643] = "X13";
+    Foo$1[Foo$1["X14"] = 13] = "X14";
+    Foo$1[Foo$1["X15"] = 4] = "X15";
+    Foo$1[Foo$1["X16"] = 9] = "X16";
+    Foo$1["X17"] = "x0";
+    Foo$1["X18"] = "0x";
+    Foo$1["X19"] = "xy";
+    Foo$1["X20"] = "NaN";
+    Foo$1["X21"] = "Infinity";
+    Foo$1["X22"] = "-Infinity";
+    Foo$1["X23"] = "0";
+    Foo$1["X24"] = "ABCD";
+    Foo$1[Foo$1["X25"] = 321] = "X25";
+    Foo$1[Foo$1["X26"] = 123] = "X26";
+    Foo$1["X27"] = "123x";
+    Foo$1["X28"] = "x123";
+    Foo$1["X29"] = "ab";
+    Foo$1[Foo$1["X30"] = Foo$1.X0] = "X30";
+    Foo$1[Foo$1["X31"] = Foo$1.X0 + "x"] = "X31";
+    Foo$1[Foo$1["X32"] = "x" + Foo$1.X0] = "X32";
+    Foo$1["X33"] = "ab";
+    Foo$1["X34"] = "x";
+    Foo$1["X35"] = "xy";
+    Foo$1["X36"] = "yx";
+    Foo$1["X37"] = "ab";
+    Foo$1[Foo$1["X38"] = Foo$1["X1"]] = "X38";
+    Foo$1[Foo$1["X39"] = Foo$1["X1"] + "y"] = "X39";
+    Foo$1[Foo$1["X40"] = "y" + Foo$1["X1"]] = "X40";
+    Foo$1["X41"] = "ab";
+    return Foo$1;
+})(Foo || ({}));
+console.log(Foo.X0, Foo.X1, Foo.X2, Foo.X3, Foo.X4, Foo.X5, Foo.X6, Foo.X7, Foo.X8, Foo.X9, Foo.X10, Foo.X11, Foo.X12, Foo.X13, Foo.X14, Foo.X15, Foo.X16, Foo.X17, Foo.X18, Foo.X19, Foo.X20, Foo.X21, Foo.X22, Foo.X23, Foo.X24, Foo.X25, Foo.X26, Foo.X27, Foo.X28, Foo.X29, Foo.X30, Foo.X31, Foo.X32, Foo.X33, Foo.X34, Foo.X35, Foo.X36, Foo.X37, Foo.X38, Foo.X39, Foo.X40, Foo.X41);

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

//#region not-supported.ts
var NonIntegerNumberToString = /* @__PURE__ */ function(NonIntegerNumberToString$1) {
	NonIntegerNumberToString$1["SUPPORTED"] = "1";
	NonIntegerNumberToString$1["UNSUPPORTED"] = "1.5";
	return NonIntegerNumberToString$1;
}(NonIntegerNumberToString || {});
console.log(NonIntegerNumberToString.SUPPORTED, NonIntegerNumberToString.UNSUPPORTED);
var OutOfBoundsNumberToString = /* @__PURE__ */ function(OutOfBoundsNumberToString$1) {
	OutOfBoundsNumberToString$1["SUPPORTED"] = "1000000000";
	OutOfBoundsNumberToString$1["UNSUPPORTED"] = "1000000000000";
	return OutOfBoundsNumberToString$1;
}(OutOfBoundsNumberToString || {});
console.log(OutOfBoundsNumberToString.SUPPORTED, OutOfBoundsNumberToString.UNSUPPORTED);
var TemplateExpressions = /* @__PURE__ */ function(TemplateExpressions$1) {
	TemplateExpressions$1[TemplateExpressions$1["NULL"] = "null"] = "NULL";
	TemplateExpressions$1[TemplateExpressions$1["TRUE"] = "true"] = "TRUE";
	TemplateExpressions$1[TemplateExpressions$1["FALSE"] = "false"] = "FALSE";
	TemplateExpressions$1[TemplateExpressions$1["BIGINT"] = "123"] = "BIGINT";
	return TemplateExpressions$1;
}(TemplateExpressions || {});
console.log(TemplateExpressions.NULL, TemplateExpressions.TRUE, TemplateExpressions.FALSE, TemplateExpressions.BIGINT);

```
### diff
```diff
===================================================================
--- esbuild	/out/not-supported.js
+++ rolldown	not-supported.js
@@ -1,13 +1,20 @@
-var NonIntegerNumberToString = (NonIntegerNumberToString2 => {
-    NonIntegerNumberToString2["SUPPORTED"] = "1";
-    NonIntegerNumberToString2["UNSUPPORTED"] = "" + 1.5;
-    return NonIntegerNumberToString2;
+var NonIntegerNumberToString = (function (NonIntegerNumberToString$1) {
+    NonIntegerNumberToString$1["SUPPORTED"] = "1";
+    NonIntegerNumberToString$1["UNSUPPORTED"] = "1.5";
+    return NonIntegerNumberToString$1;
 })(NonIntegerNumberToString || ({}));
-console.log("1", NonIntegerNumberToString.UNSUPPORTED);
-var OutOfBoundsNumberToString = (OutOfBoundsNumberToString2 => {
-    OutOfBoundsNumberToString2["SUPPORTED"] = "1000000000";
-    OutOfBoundsNumberToString2["UNSUPPORTED"] = "" + 1e12;
-    return OutOfBoundsNumberToString2;
+console.log(NonIntegerNumberToString.SUPPORTED, NonIntegerNumberToString.UNSUPPORTED);
+var OutOfBoundsNumberToString = (function (OutOfBoundsNumberToString$1) {
+    OutOfBoundsNumberToString$1["SUPPORTED"] = "1000000000";
+    OutOfBoundsNumberToString$1["UNSUPPORTED"] = "1000000000000";
+    return OutOfBoundsNumberToString$1;
 })(OutOfBoundsNumberToString || ({}));
-console.log("1000000000", OutOfBoundsNumberToString.UNSUPPORTED);
-console.log("null", "true", "false", "123");
+console.log(OutOfBoundsNumberToString.SUPPORTED, OutOfBoundsNumberToString.UNSUPPORTED);
+var TemplateExpressions = (function (TemplateExpressions$1) {
+    TemplateExpressions$1[TemplateExpressions$1["NULL"] = "null"] = "NULL";
+    TemplateExpressions$1[TemplateExpressions$1["TRUE"] = "true"] = "TRUE";
+    TemplateExpressions$1[TemplateExpressions$1["FALSE"] = "false"] = "FALSE";
+    TemplateExpressions$1[TemplateExpressions$1["BIGINT"] = "123"] = "BIGINT";
+    return TemplateExpressions$1;
+})(TemplateExpressions || ({}));
+console.log(TemplateExpressions.NULL, TemplateExpressions.TRUE, TemplateExpressions.FALSE, TemplateExpressions.BIGINT);

```