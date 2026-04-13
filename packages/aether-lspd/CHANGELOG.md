# Changelog

All notable changes to this project will be documented in this file.

## [0.1.4] - 2026-04-13

### Bug Fixes

- *(lspd)* Flaky test ([a4b1d82](https://github.com/jcarver989/aether/commit/a4b1d82b5c5fc7c51826f8ad7891046e7b1e1336))
- Fmt ([e404954](https://github.com/jcarver989/aether/commit/e4049540519573049de96594e5a918056b7f7d5e))

### Release

- V0.1.3 ([c024669](https://github.com/jcarver989/aether/commit/c024669671ec935afedd8581b165c20676d376e8))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(workspace)* Clippy ([5da71cb](https://github.com/jcarver989/aether/commit/5da71cbbbc2bb275278b8d2687add5c3c136fd02))
- *(lspd)* Stale diagnostics ([a96566b](https://github.com/jcarver989/aether/commit/a96566b9fd1c9b38220a723f06440f91b0e9f8f2))
- *(lspd)* Cannonicalize paths to resolve symlinks on mac ([8b224fb](https://github.com/jcarver989/aether/commit/8b224fb199b864937fbddaa4f5bd0c98a723d7f4))
- Tests ([65f57ff](https://github.com/jcarver989/aether/commit/65f57ff0496efcd902fc7b44e0618c9e58316cb6))
- *(lspd)* Cannonicalize root path to avoid duplicated lsp processes ([53a53d3](https://github.com/jcarver989/aether/commit/53a53d3e7abbe9321d91918404b9e9b8ff47511e))
- *(aether-lspd)* E2E tests not cleaning up daemons and causing ooms on linux ([5c94ebd](https://github.com/jcarver989/aether/commit/5c94ebdf6d2ccb257de43982a37af2ad84c9fcc6))
- *(aether-lspd)* Test ([ef83551](https://github.com/jcarver989/aether/commit/ef83551f4a39a030e68a8a66ac46eecc94e872ff))
- Lsp test ([c611d31](https://github.com/jcarver989/aether/commit/c611d31297732b4f6a506410822eb781c108cfeb))
- Clippy ([310bb21](https://github.com/jcarver989/aether/commit/310bb2171abc0c4eaf4fe7f0b089dfbacbbcc4a0))
- Clippy warnings ([b6acd52](https://github.com/jcarver989/aether/commit/b6acd52772a328da37166439b893f4abbff9293b))
- Tests ([d15b874](https://github.com/jcarver989/aether/commit/d15b8740cee262e41ceafd1ed364010507dcb86c))
- Tests ([9a847b9](https://github.com/jcarver989/aether/commit/9a847b92c46aa78672ccfe6c170cd5dd0cb89d23))
- Auto-continue agent loop if llm prematurely gives up ([4c84844](https://github.com/jcarver989/aether/commit/4c84844752071cc5284517d8eead27b64dbd8829))

### Features

- *(mcp-servers)* Prototype rename field ([824a11d](https://github.com/jcarver989/aether/commit/824a11d7b06cb106f6fa90ff5148b6befa21cc6e))
- Add MCP roots support to aether and mcp-lexicon ([eb876e7](https://github.com/jcarver989/aether/commit/eb876e7dc143bb34ca3f035c5be39044d0dccfd8))
- Lspd package for daemon ([e5bb428](https://github.com/jcarver989/aether/commit/e5bb42849b44806df672cd32c6afe441a324652f))

### Refactoring

- *(aether-lspd)* Split out refresh queue and apply better boundaries ([773bb4e](https://github.com/jcarver989/aether/commit/773bb4e31e037a3a3f552843adf2b4f919f8dae6))
- *(lspd)* Start to cleanup the lspd code by splitting up modules ([f9d0659](https://github.com/jcarver989/aether/commit/f9d0659a0c0d42a93a5d8fe751326ed237a16892))
- *(wisp)* Git diff cleanups ([49c069c](https://github.com/jcarver989/aether/commit/49c069cf134cbac2582481912f6d6d068e21269b))
- Checkpoint working lspd file watcher changes ([ce17ee0](https://github.com/jcarver989/aether/commit/ce17ee0986395302836379c4925d697854991642))
- Lsp tools ([6916652](https://github.com/jcarver989/aether/commit/691665220e0baf05fea4ce8e7ecb2e3f85349a65))
- Lsp coding tools are now split from the coding mcp server with stdio support for other agents ([bfa9e94](https://github.com/jcarver989/aether/commit/bfa9e94a9ed24cf1d993ba2010a21fa7e05e7a44))
- Absorb agent-events into aether package ([472f6c5](https://github.com/jcarver989/aether/commit/472f6c55b77c51aa8ae4ce30fea9f7f3c223bca6))
- Simplify ([1caaefb](https://github.com/jcarver989/aether/commit/1caaefbe6751401b9d3ba16be42b09b831155fe1))
