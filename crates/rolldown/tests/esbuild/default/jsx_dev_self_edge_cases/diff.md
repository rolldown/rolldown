## /out/class-this.js
### esbuild
```js
// class-this.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class {
  foo() {
    return /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "class-this.jsx",
      lineNumber: 1,
      columnNumber: 35
    }, this);
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region class-this.jsx
var _jsxFileName = "class-this.jsx";
var Foo = class {
	foo() {
		return /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 35
		}, this);
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/class-this.js
+++ rolldown	class-this.js
@@ -1,9 +1,10 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "class-this.jsx";
 var Foo = class {
     foo() {
         return jsxDEV("div", {}, void 0, false, {
-            fileName: "class-this.jsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 35
         }, this);
     }

```
## /out/derived-constructor-arg.js
### esbuild
```js
// derived-constructor-arg.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class extends Object {
  constructor(foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
    fileName: "derived-constructor-arg.jsx",
    lineNumber: 1,
    columnNumber: 53
  })) {
    super();
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region derived-constructor-arg.jsx
var _jsxFileName = "derived-constructor-arg.jsx";
var Foo = class extends Object {
	constructor(foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 53
	})) {
		super();
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-constructor-arg.js
+++ rolldown	derived-constructor-arg.js
@@ -1,8 +1,9 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "derived-constructor-arg.jsx";
 var Foo = class extends Object {
     constructor(foo = jsxDEV("div", {}, void 0, false, {
-        fileName: "derived-constructor-arg.jsx",
+        fileName: _jsxFileName,
         lineNumber: 1,
         columnNumber: 53
     })) {
         super();

```
## /out/derived-constructor-field.js
### esbuild
```js
// derived-constructor-field.tsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class extends Object {
  constructor() {
    super(...arguments);
    this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "derived-constructor-field.tsx",
      lineNumber: 1,
      columnNumber: 41
    }, this);
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region derived-constructor-field.tsx
var _jsxFileName = "derived-constructor-field.tsx";
var Foo = class extends Object {
	constructor(..._args) {
		super(..._args);
		this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 41
		});
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-constructor-field.js
+++ rolldown	derived-constructor-field.js
@@ -1,12 +1,13 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "derived-constructor-field.tsx";
 var Foo = class extends Object {
-    constructor() {
-        super(...arguments);
+    constructor(..._args) {
+        super(..._args);
         this.foo = jsxDEV("div", {}, void 0, false, {
-            fileName: "derived-constructor-field.tsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 41
-        }, this);
+        });
     }
 };
 export {Foo};

```
## /out/derived-constructor.js
### esbuild
```js
// derived-constructor.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class extends Object {
  constructor() {
    super(/* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "derived-constructor.jsx",
      lineNumber: 1,
      columnNumber: 57
    }));
    this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "derived-constructor.jsx",
      lineNumber: 1,
      columnNumber: 77
    });
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region derived-constructor.jsx
var _jsxFileName = "derived-constructor.jsx";
var Foo = class extends Object {
	constructor() {
		super(/* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 57
		}));
		this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 77
		});
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/derived-constructor.js
+++ rolldown	derived-constructor.js
@@ -1,14 +1,15 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "derived-constructor.jsx";
 var Foo = class extends Object {
     constructor() {
         super(jsxDEV("div", {}, void 0, false, {
-            fileName: "derived-constructor.jsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 57
         }));
         this.foo = jsxDEV("div", {}, void 0, false, {
-            fileName: "derived-constructor.jsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 77
         });
     }

```
## /out/function-this.js
### esbuild
```js
// function-this.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
function Foo() {
  return /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
    fileName: "function-this.jsx",
    lineNumber: 1,
    columnNumber: 32
  }, this);
}
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region function-this.jsx
var _jsxFileName = "function-this.jsx";
function Foo() {
	return /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 32
	}, this);
}

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/function-this.js
+++ rolldown	function-this.js
@@ -1,8 +1,9 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "function-this.jsx";
 function Foo() {
     return jsxDEV("div", {}, void 0, false, {
-        fileName: "function-this.jsx",
+        fileName: _jsxFileName,
         lineNumber: 1,
         columnNumber: 32
     }, this);
 }

```
## /out/normal-constructor-arg.js
### esbuild
```js
// normal-constructor-arg.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class {
  constructor(foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
    fileName: "normal-constructor-arg.jsx",
    lineNumber: 1,
    columnNumber: 38
  }, this)) {
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region normal-constructor-arg.jsx
var _jsxFileName = "normal-constructor-arg.jsx";
var Foo = class {
	constructor(foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 38
	}, this)) {}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/normal-constructor-arg.js
+++ rolldown	normal-constructor-arg.js
@@ -1,8 +1,9 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "normal-constructor-arg.jsx";
 var Foo = class {
     constructor(foo = jsxDEV("div", {}, void 0, false, {
-        fileName: "normal-constructor-arg.jsx",
+        fileName: _jsxFileName,
         lineNumber: 1,
         columnNumber: 38
     }, this)) {}
 };

```
## /out/normal-constructor-field.js
### esbuild
```js
// normal-constructor-field.tsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class {
  constructor() {
    this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "normal-constructor-field.tsx",
      lineNumber: 1,
      columnNumber: 26
    }, this);
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region normal-constructor-field.tsx
var _jsxFileName = "normal-constructor-field.tsx";
var Foo = class {
	constructor() {
		this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 26
		}, this);
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/normal-constructor-field.js
+++ rolldown	normal-constructor-field.js
@@ -1,9 +1,10 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "normal-constructor-field.tsx";
 var Foo = class {
     constructor() {
         this.foo = jsxDEV("div", {}, void 0, false, {
-            fileName: "normal-constructor-field.tsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 26
         }, this);
     }

```
## /out/normal-constructor.js
### esbuild
```js
// normal-constructor.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = class {
  constructor() {
    this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "normal-constructor.jsx",
      lineNumber: 1,
      columnNumber: 47
    }, this);
  }
};
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region normal-constructor.jsx
var _jsxFileName = "normal-constructor.jsx";
var Foo = class {
	constructor() {
		this.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
			fileName: _jsxFileName,
			lineNumber: 1,
			columnNumber: 47
		}, this);
	}
};

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/normal-constructor.js
+++ rolldown	normal-constructor.js
@@ -1,9 +1,10 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "normal-constructor.jsx";
 var Foo = class {
     constructor() {
         this.foo = jsxDEV("div", {}, void 0, false, {
-            fileName: "normal-constructor.jsx",
+            fileName: _jsxFileName,
             lineNumber: 1,
             columnNumber: 47
         }, this);
     }

```
## /out/static-field.js
### esbuild
```js
// static-field.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var _Foo = class _Foo {
};
__publicField(_Foo, "foo", /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
  fileName: "static-field.jsx",
  lineNumber: 1,
  columnNumber: 33
}, _Foo));
var Foo = _Foo;
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region static-field.jsx
var _jsxFileName = "static-field.jsx";
var _Foo;
var Foo = class {};
_Foo = Foo;
Foo.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
	fileName: _jsxFileName,
	lineNumber: 1,
	columnNumber: 33
}, _Foo);

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/static-field.js
+++ rolldown	static-field.js
@@ -1,9 +1,11 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
-var _Foo = class _Foo {};
-__publicField(_Foo, "foo", jsxDEV("div", {}, void 0, false, {
-    fileName: "static-field.jsx",
+var _jsxFileName = "static-field.jsx";
+var _Foo;
+var Foo = class {};
+_Foo = Foo;
+Foo.foo = jsxDEV("div", {}, void 0, false, {
+    fileName: _jsxFileName,
     lineNumber: 1,
     columnNumber: 33
-}, _Foo));
-var Foo = _Foo;
+}, _Foo);
 export {Foo};

```
## /out/top-level-this-cjs.js
### esbuild
```js
// top-level-this-cjs.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var require_top_level_this_cjs = __commonJS({
  "top-level-this-cjs.jsx"(exports) {
    exports.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
      fileName: "top-level-this-cjs.jsx",
      lineNumber: 1,
      columnNumber: 15
    });
  }
});
export default require_top_level_this_cjs();
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

// HIDDEN [\0rolldown/runtime.js]
//#region top-level-this-cjs.jsx
var require_top_level_this_cjs = /* @__PURE__ */ __commonJSMin(((exports) => {
	var _jsxFileName = "top-level-this-cjs.jsx";
	exports.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 15
	}, exports);
}));

//#endregion
export default require_top_level_this_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-this-cjs.js
+++ rolldown	top-level-this-cjs.js
@@ -1,11 +1,10 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
-var require_top_level_this_cjs = __commonJS({
-    "top-level-this-cjs.jsx"(exports) {
-        exports.foo = jsxDEV("div", {}, void 0, false, {
-            fileName: "top-level-this-cjs.jsx",
-            lineNumber: 1,
-            columnNumber: 15
-        });
-    }
+var require_top_level_this_cjs = __commonJSMin(exports => {
+    var _jsxFileName = "top-level-this-cjs.jsx";
+    exports.foo = jsxDEV("div", {}, void 0, false, {
+        fileName: _jsxFileName,
+        lineNumber: 1,
+        columnNumber: 15
+    }, exports);
 });
 export default require_top_level_this_cjs();

```
## /out/top-level-this-esm.js
### esbuild
```js
// top-level-this-esm.jsx
import { jsxDEV } from "react/jsx-dev-runtime";
var foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
  fileName: "top-level-this-esm.jsx",
  lineNumber: 1,
  columnNumber: 18
});
if (Foo) {
  foo = /* @__PURE__ */ jsxDEV(Foo, { children: "nested top-level this" }, void 0, false, {
    fileName: "top-level-this-esm.jsx",
    lineNumber: 1,
    columnNumber: 43
  });
}
export {
  foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region top-level-this-esm.jsx
var _jsxFileName = "top-level-this-esm.jsx";
let foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
	fileName: _jsxFileName,
	lineNumber: 1,
	columnNumber: 18
}, void 0);
if (Foo) foo = /* @__PURE__ */ jsxDEV(Foo, { children: "nested top-level this" }, void 0, false, {
	fileName: _jsxFileName,
	lineNumber: 1,
	columnNumber: 43
}, void 0);

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level-this-esm.js
+++ rolldown	top-level-this-esm.js
@@ -1,16 +1,15 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "top-level-this-esm.jsx";
 var foo = jsxDEV("div", {}, void 0, false, {
-    fileName: "top-level-this-esm.jsx",
+    fileName: _jsxFileName,
     lineNumber: 1,
     columnNumber: 18
-});
-if (Foo) {
-    foo = jsxDEV(Foo, {
-        children: "nested top-level this"
-    }, void 0, false, {
-        fileName: "top-level-this-esm.jsx",
-        lineNumber: 1,
-        columnNumber: 43
-    });
-}
+}, void 0);
+if (Foo) foo = jsxDEV(Foo, {
+    children: "nested top-level this"
+}, void 0, false, {
+    fileName: _jsxFileName,
+    lineNumber: 1,
+    columnNumber: 43
+}, void 0);
 export {foo};

```
## /out/tsconfig.js
### esbuild
```js
// tsconfig.json
var compilerOptions = { useDefineForClassFields: false };
var tsconfig_default = { compilerOptions };
export {
  compilerOptions,
  tsconfig_default as default
};
```
### rolldown
```js
//#region tsconfig.json
var compilerOptions = { "useDefineForClassFields": false };
var tsconfig_default = { compilerOptions };

//#endregion
export { compilerOptions, tsconfig_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/tsconfig.js
+++ rolldown	tsconfig.js
@@ -1,6 +1,6 @@
 var compilerOptions = {
-    useDefineForClassFields: false
+    "useDefineForClassFields": false
 };
 var tsconfig_default = {
     compilerOptions
 };

```
## /out/typescript-enum.js
### esbuild
```js
// typescript-enum.tsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo = /* @__PURE__ */ ((Foo2) => {
  Foo2[Foo2["foo"] = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
    fileName: "typescript-enum.tsx",
    lineNumber: 1,
    columnNumber: 25
  })] = "foo";
  return Foo2;
})(Foo || {});
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region typescript-enum.tsx
var _jsxFileName = "typescript-enum.tsx";
let Foo = /* @__PURE__ */ function(Foo) {
	Foo[Foo["foo"] = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 25
	}, this)] = "foo";
	return Foo;
}({});

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/typescript-enum.js
+++ rolldown	typescript-enum.js
@@ -1,10 +1,11 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
-var Foo = (Foo2 => {
-    Foo2[Foo2["foo"] = jsxDEV("div", {}, void 0, false, {
-        fileName: "typescript-enum.tsx",
+var _jsxFileName = "typescript-enum.tsx";
+var Foo = (function (Foo) {
+    Foo[Foo["foo"] = jsxDEV("div", {}, void 0, false, {
+        fileName: _jsxFileName,
         lineNumber: 1,
         columnNumber: 25
-    })] = "foo";
-    return Foo2;
-})(Foo || ({}));
+    }, this)] = "foo";
+    return Foo;
+})({});
 export {Foo};

```
## /out/typescript-namespace.js
### esbuild
```js
// typescript-namespace.tsx
import { jsxDEV } from "react/jsx-dev-runtime";
var Foo;
((Foo2) => {
  Foo2.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
    fileName: "typescript-namespace.tsx",
    lineNumber: 1,
    columnNumber: 41
  });
})(Foo || (Foo = {}));
export {
  Foo
};
```
### rolldown
```js
import { jsxDEV } from "react/jsx-dev-runtime";

//#region typescript-namespace.tsx
var _jsxFileName = "typescript-namespace.tsx";
let Foo;
(function(_Foo) {
	_Foo.foo = /* @__PURE__ */ jsxDEV("div", {}, void 0, false, {
		fileName: _jsxFileName,
		lineNumber: 1,
		columnNumber: 41
	}, this);
})(Foo || (Foo = {}));

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/typescript-namespace.js
+++ rolldown	typescript-namespace.js
@@ -1,10 +1,11 @@
 import {jsxDEV} from "react/jsx-dev-runtime";
+var _jsxFileName = "typescript-namespace.tsx";
 var Foo;
-(Foo2 => {
-    Foo2.foo = jsxDEV("div", {}, void 0, false, {
-        fileName: "typescript-namespace.tsx",
+(function (_Foo) {
+    _Foo.foo = jsxDEV("div", {}, void 0, false, {
+        fileName: _jsxFileName,
         lineNumber: 1,
         columnNumber: 41
-    });
+    }, this);
 })(Foo || (Foo = {}));
 export {Foo};

```