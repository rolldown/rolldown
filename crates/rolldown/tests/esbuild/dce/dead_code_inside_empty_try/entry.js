try { foo() }
catch { require('./a') }
finally { require('./b') }

try {}
catch { require('./c') }
finally { require('./d') }