var effect1 = () => console.log( 'effect' );
var associated = () => {};
for ( var associated in { x: 1 } ) {
	associated = effect1;
}
associated();

var effect3 = () => console.log( 'effect' );
for ( const foo in { x: effect3() } ) {
}

for ( globalThis.unknown in { x: 1 } ) {}
