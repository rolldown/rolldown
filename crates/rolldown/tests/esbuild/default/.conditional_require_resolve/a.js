require.resolve(x ? 'a' : y ? 'b' : 'c')
require.resolve(x ? y ? 'a' : 'b' : c)