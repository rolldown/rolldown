console.log( 1 );
var result;
switch ( globalThis.unknown ) {
	case 'foo':
		result = 'foo';
		break;

	default:
		result = 'default';
}
console.log( 2 );
