import * as a from "a"
import {b} from "b"
import c from "c"
a()
b()
c()
new a()
new b()
new c()

// case that ident parent is callExpr but not need to report
test(a);
