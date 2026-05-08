import React from 'react';
import { jsx } from 'react/jsx-runtime';

console.log('entry', React.version, jsx('main', {}));
import('./route.js');
