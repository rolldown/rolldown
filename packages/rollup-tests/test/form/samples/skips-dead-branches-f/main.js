var obj = {};
obj.foo = function () {
	console.log( 'this should be excluded' );
}

function bar () {
	console.log( 'this should be included' );
}

if ( 42 != '42' ) obj.foo();
bar();
