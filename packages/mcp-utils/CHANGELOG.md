# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-04-13

### Features

- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
- *(aether-cli)* Support loading and merging multiple mcp.json files ([31821b3](https://github.com/jcarver989/aether/commit/31821b3809602d46e678876ca0d65cffc0762f10))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(mcp-utils)* Don't crash process on stdio mcp startup failure ([505235d](https://github.com/jcarver989/aether/commit/505235d5d91b08255f35048565681fc56791ede3))
- *(mcp-servers)* Use similar crate to fix diffs ([859bf6d](https://github.com/jcarver989/aether/commit/859bf6d4e164e5456aa59087fcee2c49e1c10b89))
- Broken agent on API errors and show line numbers on file tools ([a4aa921](https://github.com/jcarver989/aether/commit/a4aa9214f512dd52f2de5f4805013f9692bc16d3))
- Clippy ([310bb21](https://github.com/jcarver989/aether/commit/310bb2171abc0c4eaf4fe7f0b089dfbacbbcc4a0))
- Clippy warnings ([b6acd52](https://github.com/jcarver989/aether/commit/b6acd52772a328da37166439b893f4abbff9293b))

### Features

- *(wisp)* Git diff split view ([be59521](https://github.com/jcarver989/aether/commit/be59521655db47d3ad45e2a4da2f0298369a3482))
- Prototype plan view ([1303209](https://github.com/jcarver989/aether/commit/130320904fc62871fac3caf70f2a30ba7dd60f21))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- MCP proxy for tools ([983a29a](https://github.com/jcarver989/aether/commit/983a29a74e7dbabe1e23f4d6854e50b9eb8d528f))
- MCP to proxy other MCP tools ([6e0989b](https://github.com/jcarver989/aether/commit/6e0989b503a2c6f81b0d997db5a41c9a5538ddeb))
- Diffs in edit tool output ([fd4216c](https://github.com/jcarver989/aether/commit/fd4216c99b96bd13760cf01f8dc1a9d12ffcaa8a))
- Improved tooloutput for agents ([cc5920f](https://github.com/jcarver989/aether/commit/cc5920fdb2545229e707355edf2ef4cf1a156792))
- Support agent model switching ([794741a](https://github.com/jcarver989/aether/commit/794741a3442dfa82e9220e720d6313b87b65f381))

### Refactoring

- *(wisp)* Make wisp support acp Diff tool content ([2068e30](https://github.com/jcarver989/aether/commit/2068e306f9421182e65c4d03ce355f222929e83e))
- Move oauth module into llm package ([396109f](https://github.com/jcarver989/aether/commit/396109f921e3c6e8ef1f99e06755278034a4714a))
- Mcp config with connection.rs ([79369ef](https://github.com/jcarver989/aether/commit/79369efcfaf9ef45ca3ad99bca43107d10fb21c4))
- Absorb agent-events into aether package ([472f6c5](https://github.com/jcarver989/aether/commit/472f6c55b77c51aa8ae4ce30fea9f7f3c223bca6))
- Rename agent/ module to core/ ([f204272](https://github.com/jcarver989/aether/commit/f204272c11d66c6da7802d4f83f6c7b465e3b133))
- Cleanup dead code, extract tool call collector and cleanup deps ([a22edde](https://github.com/jcarver989/aether/commit/a22edde2c2c99981243096416ed788cc0813221e))
- Split MCPs into their own crates ([4333dc9](https://github.com/jcarver989/aether/commit/4333dc95632b5fc6ccb7427a18fcc5c8c46e8b90))

### Refaactor

- Extract connection.rs for mcp ([ddcc192](https://github.com/jcarver989/aether/commit/ddcc192f09d9eb10e8f965fe4e5b1ad668eae649))
