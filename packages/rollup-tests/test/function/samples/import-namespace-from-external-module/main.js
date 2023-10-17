import * as path from 'path';

var path1 = 'foo/bar/baz';
var path2 = 'foo/baz/bar';

assert.equal( path.relative( path1, path2 ), path.normalize('../../baz/bar') );