# Diff
## /out.js
### esbuild
```js
// entry.ts
var A_keep = class {
  static {
    foo;
  }
};
var B_keep = class {
  static {
    this.foo;
  }
};
var C_keep = class {
  static {
    try {
      foo;
    } catch {
    }
  }
};
var D_keep = class {
  static {
    try {
    } finally {
      foo;
    }
  }
};
```
### rolldown
```js

//#region entry.ts
class A_keep {
	static {
		foo;
	}
}
class B_keep {
	static {
		this.foo;
	}
}
class C_keep {
	static {
		try {
			foo;
		} catch {}
	}
}
class D_keep {
	static {
		try {} finally {
			foo;
		}
	}
}

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,24 +1,24 @@
-var A_keep = class {
+class A_keep {
     static {
         foo;
     }
-};
-var B_keep = class {
+}
+class B_keep {
     static {
         this.foo;
     }
-};
-var C_keep = class {
+}
+class C_keep {
     static {
         try {
             foo;
         } catch {}
     }
-};
-var D_keep = class {
+}
+class D_keep {
     static {
         try {} finally {
             foo;
         }
     }
-};
+}

```