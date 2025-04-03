# Reason
1. not support const enum inline
# Diff
## /out.js
### esbuild
```js
// entry.ts
var Foo = class {
  100 = 100;
  200 = 200;
  300 = 300;
  "str 1" = "str 1" /* STR */;
  123 = 123 /* NUM */;
  "str 2" = "str 2" /* STR */;
  321 = 321 /* NUM */;
};
shouldNotBeComputed(
  class {
    100 = 100;
    200 = 200;
    300 = 300;
    "str 1" = "str 1" /* STR */;
    123 = 123 /* NUM */;
    "str 2" = "str 2" /* STR */;
    321 = 321 /* NUM */;
  },
  {
    100: 100,
    200: 200,
    300: 300,
    "str 1": "str 1" /* STR */,
    123: 123 /* NUM */,
    "str 2": "str 2" /* STR */,
    321: 321 /* NUM */
  }
);
mustBeComputed(
  { ["__proto__"]: null },
  { ["__proto__"]: null },
  class {
    ["constructor"]() {
    }
  },
  class {
    ["constructor"]() {
    }
  },
  class {
    static ["prototype"]() {
    }
  },
  class {
    static ["prototype"]() {
    }
  }
);
```
### rolldown
```js

//#region cross-file.ts
let CrossFileGood = /* @__PURE__ */ function(CrossFileGood) {
	CrossFileGood["STR"] = "str 2";
	CrossFileGood[CrossFileGood["NUM"] = 321] = "NUM";
	return CrossFileGood;
}({});
let CrossFileBad$1 = /* @__PURE__ */ function(CrossFileBad) {
	CrossFileBad["PROTO"] = "__proto__";
	CrossFileBad["CONSTRUCTOR"] = "constructor";
	CrossFileBad["PROTOTYPE"] = "prototype";
	return CrossFileBad;
}({});

//#endregion
//#region entry.ts
var SameFileGood = /* @__PURE__ */ function(SameFileGood) {
	SameFileGood["STR"] = "str 1";
	SameFileGood[SameFileGood["NUM"] = 123] = "NUM";
	return SameFileGood;
}(SameFileGood || {});
var SameFileBad = /* @__PURE__ */ function(SameFileBad) {
	SameFileBad["PROTO"] = "__proto__";
	SameFileBad["CONSTRUCTOR"] = "constructor";
	SameFileBad["PROTOTYPE"] = "prototype";
	return SameFileBad;
}(SameFileBad || {});
var Foo = class {
	[100] = 100;
	"200" = 200;
	["300"] = 300;
	[SameFileGood.STR] = SameFileGood.STR;
	[SameFileGood.NUM] = SameFileGood.NUM;
	[CrossFileGood.STR] = CrossFileGood.STR;
	[CrossFileGood.NUM] = CrossFileGood.NUM;
};
shouldNotBeComputed(class {
	[100] = 100;
	"200" = 200;
	["300"] = 300;
	[SameFileGood.STR] = SameFileGood.STR;
	[SameFileGood.NUM] = SameFileGood.NUM;
	[CrossFileGood.STR] = CrossFileGood.STR;
	[CrossFileGood.NUM] = CrossFileGood.NUM;
}, {
	[100]: 100,
	"200": 200,
	["300"]: 300,
	[SameFileGood.STR]: SameFileGood.STR,
	[SameFileGood.NUM]: SameFileGood.NUM,
	[CrossFileGood.STR]: CrossFileGood.STR,
	[CrossFileGood.NUM]: CrossFileGood.NUM
});
mustBeComputed({ [SameFileBad.PROTO]: null }, { [CrossFileBad$1.PROTO]: null }, class {
	[SameFileBad.CONSTRUCTOR]() {}
}, class {
	[CrossFileBad$1.CONSTRUCTOR]() {}
}, class {
	static [SameFileBad.PROTOTYPE]() {}
}, class {
	static [CrossFileBad$1.PROTOTYPE]() {}
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,39 +1,61 @@
+var CrossFileGood = (function (CrossFileGood) {
+    CrossFileGood["STR"] = "str 2";
+    CrossFileGood[CrossFileGood["NUM"] = 321] = "NUM";
+    return CrossFileGood;
+})({});
+var CrossFileBad$1 = (function (CrossFileBad) {
+    CrossFileBad["PROTO"] = "__proto__";
+    CrossFileBad["CONSTRUCTOR"] = "constructor";
+    CrossFileBad["PROTOTYPE"] = "prototype";
+    return CrossFileBad;
+})({});
+var SameFileGood = (function (SameFileGood) {
+    SameFileGood["STR"] = "str 1";
+    SameFileGood[SameFileGood["NUM"] = 123] = "NUM";
+    return SameFileGood;
+})(SameFileGood || ({}));
+var SameFileBad = (function (SameFileBad) {
+    SameFileBad["PROTO"] = "__proto__";
+    SameFileBad["CONSTRUCTOR"] = "constructor";
+    SameFileBad["PROTOTYPE"] = "prototype";
+    return SameFileBad;
+})(SameFileBad || ({}));
 var Foo = class {
-    100 = 100;
-    200 = 200;
-    300 = 300;
-    "str 1" = "str 1";
-    123 = 123;
-    "str 2" = "str 2";
-    321 = 321;
+    [100] = 100;
+    "200" = 200;
+    ["300"] = 300;
+    [SameFileGood.STR] = SameFileGood.STR;
+    [SameFileGood.NUM] = SameFileGood.NUM;
+    [CrossFileGood.STR] = CrossFileGood.STR;
+    [CrossFileGood.NUM] = CrossFileGood.NUM;
 };
 shouldNotBeComputed(class {
-    100 = 100;
-    200 = 200;
-    300 = 300;
-    "str 1" = "str 1";
-    123 = 123;
-    "str 2" = "str 2";
-    321 = 321;
+    [100] = 100;
+    "200" = 200;
+    ["300"] = 300;
+    [SameFileGood.STR] = SameFileGood.STR;
+    [SameFileGood.NUM] = SameFileGood.NUM;
+    [CrossFileGood.STR] = CrossFileGood.STR;
+    [CrossFileGood.NUM] = CrossFileGood.NUM;
 }, {
-    100: 100,
-    200: 200,
-    300: 300,
-    "str 1": "str 1",
-    123: 123,
-    "str 2": "str 2",
-    321: 321
+    [100]: 100,
+    "200": 200,
+    ["300"]: 300,
+    [SameFileGood.STR]: SameFileGood.STR,
+    [SameFileGood.NUM]: SameFileGood.NUM,
+    [CrossFileGood.STR]: CrossFileGood.STR,
+    [CrossFileGood.NUM]: CrossFileGood.NUM
 });
 mustBeComputed({
-    ["__proto__"]: null
+    [SameFileBad.PROTO]: null
 }, {
-    ["__proto__"]: null
+    [CrossFileBad$1.PROTO]: null
 }, class {
-    ["constructor"]() {}
+    [SameFileBad.CONSTRUCTOR]() {}
 }, class {
-    ["constructor"]() {}
+    [CrossFileBad$1.CONSTRUCTOR]() {}
 }, class {
-    static ["prototype"]() {}
+    static [SameFileBad.PROTOTYPE]() {}
 }, class {
-    static ["prototype"]() {}
+    static [CrossFileBad$1.PROTOTYPE]() {}
 });

```