This handler will not be invoked if logs are filtered out by the [`logLevel`](/reference/InputOptions.logLevel) option. I.e. by default, `"debug"` logs will be swallowed.

If the default handler is not invoked, the log will not be printed to the console. Moreover, you can change the log level by invoking the default handler with a different level. Using the additional level `"error"` will turn the log into a thrown error that has all properties of the log attached.
