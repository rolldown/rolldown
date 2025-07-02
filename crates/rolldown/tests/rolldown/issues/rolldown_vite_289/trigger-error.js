// This file is used to trigger an error if rolldown doesn't include the module correctly.
import nodeAssert from 'node:assert';
import { foo } from "./lib-index";


nodeAssert(foo() === 1, 'foo() should return 1');