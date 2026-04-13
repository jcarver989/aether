# Changelog

All notable changes to this project will be documented in this file.

## [0.1.4] - 2026-04-13

### Features

- *(aether-cli)* Support shell command expansion in prompt files ([f3ea6f5](https://github.com/jcarver989/aether/commit/f3ea6f56142f35354f585082edfd3196a84a6547))
- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
- *(aether-cli)* Support loading and merging multiple mcp.json files ([31821b3](https://github.com/jcarver989/aether/commit/31821b3809602d46e678876ca0d65cffc0762f10))

### Release

- V0.1.3 ([c024669](https://github.com/jcarver989/aether/commit/c024669671ec935afedd8581b165c20676d376e8))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(workspace)* Clippy ([5da71cb](https://github.com/jcarver989/aether/commit/5da71cbbbc2bb275278b8d2687add5c3c136fd02))
- *(llm)* Make provider factory async to avoid block call in Bedrock ([44e727a](https://github.com/jcarver989/aether/commit/44e727afd19313f296071cc7862ed993d5b1af6e))
- *(core)* Handle streaming tool updates ([bb2cc81](https://github.com/jcarver989/aether/commit/bb2cc81a37364b8662b3a2c51683909d3afcd928))
- *(aether-core)* Not triggering handoff on FinishReason for max token length exceeded ([582c2d4](https://github.com/jcarver989/aether/commit/582c2d48973cee6b60153a867a0380e88594abb0))
- Broken agent on API errors and show line numbers on file tools ([a4aa921](https://github.com/jcarver989/aether/commit/a4aa9214f512dd52f2de5f4805013f9692bc16d3))
- Missing method in tests ([51ff951](https://github.com/jcarver989/aether/commit/51ff95131e658b25b58a740784ce5fdb880d95db))

### Features

- *(wisp)* Git diff split view ([be59521](https://github.com/jcarver989/aether/commit/be59521655db47d3ad45e2a4da2f0298369a3482))
- *(aether)* Support prompts with images and audio ([f58754d](https://github.com/jcarver989/aether/commit/f58754d803f9458c34232756f2acc2ab59ff7183))
- *(aether-cli)* Support filtering tools for sub-agents in settings file ([56a13b4](https://github.com/jcarver989/aether/commit/56a13b4189bc6b8218c2fa6dca845e452af7dcdd))
- *(aether-cli)* Session resume ([b8d225e](https://github.com/jcarver989/aether/commit/b8d225e88c13ddefedce58ce3e71ef27dc8e115b))
- *(llm)* Pass encrypted reasoning chunks to codex models ([4dd22bf](https://github.com/jcarver989/aether/commit/4dd22bfaf8165796c9edf631828cd93b92babd72))
- *(aether)* New settings.json format for sub-agents ([9371004](https://github.com/jcarver989/aether/commit/9371004c1685658fcc0e331762853a2936a96f11))
- *(aether-cli)* Customizable, composible prompts via globs ([8b858b6](https://github.com/jcarver989/aether/commit/8b858b60e056d8851182e771aac9c45fc118472d))
- Set reasoning level for supported models ([ba19fe1](https://github.com/jcarver989/aether/commit/ba19fe18fe9773aa79f5a2d9837dcc02e5d2bf32))
- Spill large tool calls to disk to preserve context ([af7887b](https://github.com/jcarver989/aether/commit/af7887b410bcd6dffbe7f3877192de98ba96706f))
- Convert mcp json output to yaml before feeding to model for token efficiency ([6c9f0a2](https://github.com/jcarver989/aether/commit/6c9f0a24849567826591ee945700e0aa15cf98ac))
- Prototype plan view ([1303209](https://github.com/jcarver989/aether/commit/130320904fc62871fac3caf70f2a30ba7dd60f21))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- MCP proxy for tools ([983a29a](https://github.com/jcarver989/aether/commit/983a29a74e7dbabe1e23f4d6854e50b9eb8d528f))
- MCP to proxy other MCP tools ([6e0989b](https://github.com/jcarver989/aether/commit/6e0989b503a2c6f81b0d997db5a41c9a5538ddeb))

### Refactoring

- *(aether-project)* Start to clean up catalog ([a426292](https://github.com/jcarver989/aether/commit/a42629213bd928168da0cc67f3bed480bfcb8a13))
- *(wisp)* Make wisp support acp Diff tool content ([2068e30](https://github.com/jcarver989/aether/commit/2068e306f9421182e65c4d03ce355f222929e83e))

### Feaet

- *(aether-cli)* Session logs and session restoration ([7b2649b](https://github.com/jcarver989/aether/commit/7b2649bfc0a1cbb54aeb8d2fa8fa5464a4e3ad77))
