// This will be tree-shaken to an empty namespace object
import('./lib').then(({unused}) => {});
