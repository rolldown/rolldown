import path from 'node:path';
import { assertNoCircularImports } from '../../../../../assert_no_circular_imports.mjs';

assertNoCircularImports(path.join(import.meta.dirname, 'dist'));
