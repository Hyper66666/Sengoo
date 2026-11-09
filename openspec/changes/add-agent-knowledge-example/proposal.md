# Add a local MCP knowledge consumer

Exercise Sengoo's new MCP/schema SDK and existing owned SQLite package in an
independent application. A trusted host selects a local root, the application
indexes bounded UTF-8 files transactionally, and an MCP client queries stored
text or safely reads current files. This is a complete standalone consumer,
not another Agent loop or an AI/RAG service.

The implementation is Sengoo. Existing generic filesystem snapshots guard reads;
no native code, credentials, external service or user state is introduced.
Windows first; synchronous directory/stdout limitations remain explicit.
