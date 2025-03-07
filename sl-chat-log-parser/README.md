# sl-chat-log-parser

Parser for SL viewer chat logs (Firestorm in particular but might work for others too)

Tries to parse every possible line into something useful, the naming of the enum variants
might still need some work to get into a more unified style but the hard part is done and
it parses my own 15+ years of chat logs.

It is highly recommended to use release builds to parse large amounts of logs, there is
a significant performance difference between debug and release builds.

Some performance optimization by reordering the parsers could probably be done but I haven't
done so yet.
