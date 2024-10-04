foo("_keepThisProperty");
foo((x, "_keepThisProperty"));
foo(x ? "_keepThisProperty" : "_keepThisPropertyToo");
x[foo("_keepThisProperty")];
x?.[foo("_keepThisProperty")];
({ [foo("_keepThisProperty")]: x });
(class { [foo("_keepThisProperty")] = x });
var { [foo("_keepThisProperty")]: x } = y;
foo("_keepThisProperty") in x;