# Changelog

All notable changes to this project will be documented in this file.

## [0.1.4] - 2026-04-13

### Bug Fixes

- *(aether-cli)* Make --sandbox-image work with tty ([58bd966](https://github.com/jcarver989/aether/commit/58bd9662c89f4d76426098321bf60e093c2f0c98))

### Features

- *(aether-cli)* Add aether agent list and aether agent remove commands ([be26268](https://github.com/jcarver989/aether/commit/be26268fca64ab9b902ae62381807ba4c82948ac))
- *(aether-cli)* Add 'aether agent new' cli command and onboarding ([eb8b22d](https://github.com/jcarver989/aether/commit/eb8b22ddf6ea37f32496d0c557c82e3681979ef5))
- *(aether-cli)* Emit more message variants in  headless mode and --events to filter emitted events ([76bb6fd](https://github.com/jcarver989/aether/commit/76bb6fd5f2296e79ca6839d705379a09c8e5d58f))
- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
- *(aether-cli)* Support loading and merging multiple mcp.json files ([31821b3](https://github.com/jcarver989/aether/commit/31821b3809602d46e678876ca0d65cffc0762f10))

### Release

- V0.1.3 ([c024669](https://github.com/jcarver989/aether/commit/c024669671ec935afedd8581b165c20676d376e8))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(llm)* Make provider factory async to avoid block call in Bedrock ([44e727a](https://github.com/jcarver989/aether/commit/44e727afd19313f296071cc7862ed993d5b1af6e))
- *(wisp)* Update provider login status when it changes ([be42db2](https://github.com/jcarver989/aether/commit/be42db27ae39eebb020846fa6ea86d7242721e3a))
- *(aether)* Support correct reasoning levels per model ([ef6e801](https://github.com/jcarver989/aether/commit/ef6e801258f10341f58febd1ce2dc687f134054d))
- *(wisp)* Remove duplicated clear command as one came from wisp and another from aether-cli ([f556657](https://github.com/jcarver989/aether/commit/f556657ca1471150e019e7aa93e9de5a57d2e009))
- *(core)* Handle streaming tool updates ([bb2cc81](https://github.com/jcarver989/aether/commit/bb2cc81a37364b8662b3a2c51683909d3afcd928))
- *(aether-cli)* Make list sessions include a better title (1st line of prompt) ([b364423](https://github.com/jcarver989/aether/commit/b364423881bf27703759bedbb046524a9a7be629))
- *(aether-cli)* Preserve order of agents defined in settings when cycling modes ([82f1a42](https://github.com/jcarver989/aether/commit/82f1a425630c5b2e14bf07e764ce9ef37e6b7133))
- *(aether-cli)* Reasoning selection was switching model ([0cfbfc6](https://github.com/jcarver989/aether/commit/0cfbfc6a452b23809abc217cf9a2219c451c67a5))

### Features

- *(aether-cli)* New agent command ([0555c9c](https://github.com/jcarver989/aether/commit/0555c9cede7f4a586d9420cf8978104a56ffa83c))
- *(wisp)* Support drag and drop images and display model capabilities in selector ([74361e5](https://github.com/jcarver989/aether/commit/74361e51f42bdfb7919646f8c44b5ce652c27b81))
- *(aether)* Support prompts with images and audio ([f58754d](https://github.com/jcarver989/aether/commit/f58754d803f9458c34232756f2acc2ab59ff7183))
- *(cli)* Allow specifying named agents in headless ([2523e02](https://github.com/jcarver989/aether/commit/2523e02e2c66d531ab2bbc70f979c856421f4be5))
- *(cli)* Give show-prompt a -a/--agent flag so you can see tokens per agent ([939e9b0](https://github.com/jcarver989/aether/commit/939e9b068083b36b1a6f670abfdbdc4116a1f482))
- *(aether-cli)* Use OS keychain for secrets storage ([de6b2cd](https://github.com/jcarver989/aether/commit/de6b2cd17ac08155f83771b002d90db2edb067f5))
- *(aether)* Auto-discover local models via http endpoint ([8a3eca3](https://github.com/jcarver989/aether/commit/8a3eca33428e167b23f35ea699c82616bb36c51a))
- *(aether-cli)* Support filtering tools for sub-agents in settings file ([56a13b4](https://github.com/jcarver989/aether/commit/56a13b4189bc6b8218c2fa6dca845e452af7dcdd))
- *(aether-cli)* Integrate wisp as dep so you can launch straight into tui with 1 command ([dc78351](https://github.com/jcarver989/aether/commit/dc7835170784368530e1a89734c3bb876a6d3013))
- *(tui)* Prototype diff split-view ([38b11fe](https://github.com/jcarver989/aether/commit/38b11feffaf2b453e04a696e1f086ceaba662151))
- *(aether-cli)* Add sub-command to output full prompt and tools for token diagnostics ([72ee80e](https://github.com/jcarver989/aether/commit/72ee80e52f2e336254b8f88aea90dc9b97c121a8))
- *(aether-cli)* Prototype sandbox mode ([5300f6c](https://github.com/jcarver989/aether/commit/5300f6c87fb164572660513aa81d466e566ac148))
- *(mcp-servers)* Unify slash commands, skills and rules ([17737e4](https://github.com/jcarver989/aether/commit/17737e46f0fa10fe26f79a893f877ada529fcb33))
- *(aether-cli)* Session resume ([b8d225e](https://github.com/jcarver989/aether/commit/b8d225e88c13ddefedce58ce3e71ef27dc8e115b))
- *(aether)* New settings.json format for sub-agents ([9371004](https://github.com/jcarver989/aether/commit/9371004c1685658fcc0e331762853a2936a96f11))
- *(aether-cli)* Customizable, composible prompts via globs ([8b858b6](https://github.com/jcarver989/aether/commit/8b858b60e056d8851182e771aac9c45fc118472d))
- *(mcp-servers)* Show tool progress meta display ([7cd250c](https://github.com/jcarver989/aether/commit/7cd250c9f9edf283eaf980004e3a8b4ba0402035))
- *(wisp)* Theme selection menu ([71fc46c](https://github.com/jcarver989/aether/commit/71fc46cf9620436a9bcec407e975bc16ff3c3c91))
- Add settings file support to aether-cli ([6569f21](https://github.com/jcarver989/aether/commit/6569f216ead8a48245ebb7f6fd2e2f588791d46f))
- Set reasoning level for supported models ([ba19fe1](https://github.com/jcarver989/aether/commit/ba19fe18fe9773aa79f5a2d9837dcc02e5d2bf32))
- Prototype provider login from wisp ui ([a87bb7e](https://github.com/jcarver989/aether/commit/a87bb7e3b59a6ab7687f5166abf2d0fbaea88ff6))
- Prototype codex support ([4685807](https://github.com/jcarver989/aether/commit/4685807e91bcda86feef2e3d2584919fe620da04))
- Prototype plan view ([1303209](https://github.com/jcarver989/aether/commit/130320904fc62871fac3caf70f2a30ba7dd60f21))
- Support alloyed models from wisp ui ([1e45195](https://github.com/jcarver989/aether/commit/1e451955253478e64cab7acd40263edd9b3c23e1))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- MCP proxy for tools ([983a29a](https://github.com/jcarver989/aether/commit/983a29a74e7dbabe1e23f4d6854e50b9eb8d528f))
- Diffs in edit tool output ([fd4216c](https://github.com/jcarver989/aether/commit/fd4216c99b96bd13760cf01f8dc1a9d12ffcaa8a))
- Improved tooloutput for agents ([cc5920f](https://github.com/jcarver989/aether/commit/cc5920fdb2545229e707355edf2ef4cf1a156792))
- Breakout aether-cli ([80d65a3](https://github.com/jcarver989/aether/commit/80d65a3efd4eba3e560dfa7cf4d7731aeb5ba363))

### Refactoring

- *(aether-project)* Start to clean up catalog ([a426292](https://github.com/jcarver989/aether/commit/a42629213bd928168da0cc67f3bed480bfcb8a13))
- *(wisp)* Make wisp support acp Diff tool content ([2068e30](https://github.com/jcarver989/aether/commit/2068e306f9421182e65c4d03ce355f222929e83e))

### Feaet

- *(aether-cli)* Session logs and session restoration ([7b2649b](https://github.com/jcarver989/aether/commit/7b2649bfc0a1cbb54aeb8d2fa8fa5464a4e3ad77))
