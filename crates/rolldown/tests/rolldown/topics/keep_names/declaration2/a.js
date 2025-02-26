export var delay = function(time){}

export function random64() {
    return (
        (BigInt((Math.random() * 0xffff_ffff) & 0xffff_ffff)<<32n)
        & (BigInt((Math.random() * 0xffff_ffff) & 0xffff_ffff))
    );
}


