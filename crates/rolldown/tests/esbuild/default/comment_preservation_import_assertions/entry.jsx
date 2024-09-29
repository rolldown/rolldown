import 'foo' /* before */ assert { type: 'json' }
import 'foo' assert /* before */ { type: 'json' }
import 'foo' assert { /* before */ type: 'json' }
import 'foo' assert { type: /* before */ 'json' }
import 'foo' assert { type: 'json' /* before */ }