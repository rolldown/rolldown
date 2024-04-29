// These operators may have side effects
let keep;
+keep;
-keep;
~keep;
delete keep;
++keep;
--keep;
keep++;
keep--;

// These operators never have side effects
let REMOVE;
!REMOVE;
void REMOVE;