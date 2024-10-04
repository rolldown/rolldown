import('foo')
import('foo')
import('foo').catch()
import('foo').catch()

import('bar').catch()
import('bar').catch()
import('bar') // We should get an error report here even though the earlier imports have the "HandlesImportErrors" flag
import('bar')

import('baz').catch()
import('baz').catch()