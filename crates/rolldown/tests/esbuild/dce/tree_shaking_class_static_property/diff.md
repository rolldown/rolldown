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
(class {
	static x = x;
});
(class {
	static ["x"] = x;
});
(class {
	static [x] = "x";
});
(class {
	static [x]() {}
});
(class {
	static get [x]() {}
});
(class {
	static set [x](_) {}
});
(class {
	static async [x]() {}
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,26 +1,21 @@
-let keep1 = class {
+(class {
     static x = x;
-};
-let keep2 = class {
+});
+(class {
     static ["x"] = x;
-};
-let keep3 = class {
+});
+(class {
     static [x] = "x";
-};
-let keep4 = class {
+});
+(class {
     static [x]() {}
-};
-let keep5 = class {
+});
+(class {
     static get [x]() {}
-};
-let keep6 = class {
+});
+(class {
     static set [x](_) {}
-};
-let keep7 = class {
+});
+(class {
     static async [x]() {}
-};
-let keep8 = class {
-    static [{
-        toString() {}
-    }] = "x";
-};
+});

```