# Diff
## /out.js
### esbuild
```js
let keep1 = class {
  static x = x;
};
let keep2 = class {
  static ["x"] = x;
};
let keep3 = class {
  static [x] = "x";
};
let keep4 = class {
  static [x]() {
  }
};
let keep5 = class {
  static get [x]() {
  }
};
let keep6 = class {
  static set [x](_) {
  }
};
let keep7 = class {
  static async [x]() {
  }
};
let keep8 = class {
  static [{ toString() {
  } }] = "x";
};
```
### rolldown
```js

//#region entry.js
let keep1 = class {
	static x = x;
};
let keep2 = class {
	static ["x"] = x;
};
let keep3 = class {
	static [x] = "x";
};
let keep4 = class {
	static [x]() {}
};
let keep5 = class {
	static get [x]() {}
};
let keep6 = class {
	static set [x](_) {}
};
let keep7 = class {
	static async [x]() {}
};
let keep8 = class {
	static [{ toString() {} }] = "x";
};

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,26 +1,26 @@
-let keep1 = class {
+var keep1 = class {
     static x = x;
 };
-let keep2 = class {
+var keep2 = class {
     static ["x"] = x;
 };
-let keep3 = class {
+var keep3 = class {
     static [x] = "x";
 };
-let keep4 = class {
+var keep4 = class {
     static [x]() {}
 };
-let keep5 = class {
+var keep5 = class {
     static get [x]() {}
 };
-let keep6 = class {
+var keep6 = class {
     static set [x](_) {}
 };
-let keep7 = class {
+var keep7 = class {
     static async [x]() {}
 };
-let keep8 = class {
+var keep8 = class {
     static [{
         toString() {}
     }] = "x";
 };

```