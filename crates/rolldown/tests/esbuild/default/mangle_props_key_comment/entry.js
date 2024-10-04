x(/* __KEY__ */ '_doNotMangleThis', /* __KEY__ */ `_doNotMangleThis`)
x._mangleThis(/* @__KEY__ */ '_mangleThis', /* @__KEY__ */ `_mangleThis`)
x._mangleThisToo(/* #__KEY__ */ '_mangleThisToo', /* #__KEY__ */ `_mangleThisToo`)
x._someKey = /* #__KEY__ */ '_someKey' in y
x([
    `foo.${/* @__KEY__ */ '_mangleThis'} = bar.${/* @__KEY__ */ '_mangleThisToo'}`,
    `foo.${/* @__KEY__ */ 'notMangled'} = bar.${/* @__KEY__ */ 'notMangledEither'}`,
])
