import React from 'react';
import { jsx } from 'react/jsx-runtime';

console.log('route', React.version, jsx('route', {}));
import('./child.js');
