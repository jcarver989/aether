# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-04-13

### Bug Fixes

- *(wisp)* Diff line style bleeding into the next line's gutter when wrapped ([a77ad1b](https://github.com/jcarver989/aether/commit/a77ad1bc3902191c14be60fdaafa83985e7cb80e))

### Features

- *(llm)* Improved token usage tracking with reasoning, cached, and multi modal ([d4d6431](https://github.com/jcarver989/aether/commit/d4d6431abce31cfd5c349cfbea8aa502860a34ae))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(workspace)* Clippy ([5da71cb](https://github.com/jcarver989/aether/commit/5da71cbbbc2bb275278b8d2687add5c3c136fd02))
- *(llm)* Make provider factory async to avoid block call in Bedrock ([44e727a](https://github.com/jcarver989/aether/commit/44e727afd19313f296071cc7862ed993d5b1af6e))
- *(aether)* Support correct reasoning levels per model ([ef6e801](https://github.com/jcarver989/aether/commit/ef6e801258f10341f58febd1ce2dc687f134054d))
- *(llm)* Set cache control for OpenRouter ([003169d](https://github.com/jcarver989/aether/commit/003169dfb0c0be6b7e3aba68cda572bd5eeeb158))
- *(llm)* Add handling for NetworkError stop reason ([097bb4c](https://github.com/jcarver989/aether/commit/097bb4c4fba8cad34c87754eec675e8e39e2726b))
- *(llm)* Token tracking for openai compatable providers ([ffdf11c](https://github.com/jcarver989/aether/commit/ffdf11ca4a6c98842a15f002e53c6a85e506ad80))
- *(llm)* Test error ([c0dc9e6](https://github.com/jcarver989/aether/commit/c0dc9e6c87d54f68f83fffa3427779323fdac7a9))
- *(aether-core)* Not triggering handoff on FinishReason for max token length exceeded ([582c2d4](https://github.com/jcarver989/aether/commit/582c2d48973cee6b60153a867a0380e88594abb0))
- *(llm)* Test ([e113664](https://github.com/jcarver989/aether/commit/e113664b0183a6ce161b59943e6c86e82a8bd7e5))
- Alloyed models that require reasoning to be set ([0eb0f7d](https://github.com/jcarver989/aether/commit/0eb0f7d736652567926f20b3348f2e9d91cf927b))
- Broken agent on API errors and show line numbers on file tools ([a4aa921](https://github.com/jcarver989/aether/commit/a4aa9214f512dd52f2de5f4805013f9692bc16d3))
- Clippy ([310bb21](https://github.com/jcarver989/aether/commit/310bb2171abc0c4eaf4fe7f0b089dfbacbbcc4a0))
- Clippy warnings ([b6acd52](https://github.com/jcarver989/aether/commit/b6acd52772a328da37166439b893f4abbff9293b))
- Constructors taking self, which they should not ([839a170](https://github.com/jcarver989/aether/commit/839a170b368564a541e7d298c3add970734b9187))

### Features

- *(aether)* Support prompts with images and audio ([f58754d](https://github.com/jcarver989/aether/commit/f58754d803f9458c34232756f2acc2ab59ff7183))
- *(llm)* OpenAI provider ([c4c4a37](https://github.com/jcarver989/aether/commit/c4c4a374dc93b3e5cfcb9b76b74087d7ea965a68))
- *(aether-cli)* Use OS keychain for secrets storage ([de6b2cd](https://github.com/jcarver989/aether/commit/de6b2cd17ac08155f83771b002d90db2edb067f5))
- *(aether)* Auto-discover local models via http endpoint ([8a3eca3](https://github.com/jcarver989/aether/commit/8a3eca33428e167b23f35ea699c82616bb36c51a))
- *(aether-cli)* Support filtering tools for sub-agents in settings file ([56a13b4](https://github.com/jcarver989/aether/commit/56a13b4189bc6b8218c2fa6dca845e452af7dcdd))
- *(tui)* Prototype diff split-view ([38b11fe](https://github.com/jcarver989/aether/commit/38b11feffaf2b453e04a696e1f086ceaba662151))
- *(aether-cli)* Session resume ([b8d225e](https://github.com/jcarver989/aether/commit/b8d225e88c13ddefedce58ce3e71ef27dc8e115b))
- *(llm)* Pass encrypted reasoning chunks to codex models ([4dd22bf](https://github.com/jcarver989/aether/commit/4dd22bfaf8165796c9edf631828cd93b92babd72))
- *(wisp)* Keyboard shortcut for reasoning level ([663326c](https://github.com/jcarver989/aether/commit/663326ca11b1392a3f4965e4d909c694d0cacb51))
- *(llm)* Add gpt-5.4 to codex provider ([43b1da7](https://github.com/jcarver989/aether/commit/43b1da75bc0880535cad98c29a434c8abf77c31b))
- Set reasoning level for supported models ([ba19fe1](https://github.com/jcarver989/aether/commit/ba19fe18fe9773aa79f5a2d9837dcc02e5d2bf32))
- Spill large tool calls to disk to preserve context ([af7887b](https://github.com/jcarver989/aether/commit/af7887b410bcd6dffbe7f3877192de98ba96706f))
- Prototype codex support ([4685807](https://github.com/jcarver989/aether/commit/4685807e91bcda86feef2e3d2584919fe620da04))
- Add support for Bedrock ([15943f5](https://github.com/jcarver989/aether/commit/15943f56d89bc3179a81281cf6ccb8557295d99b))
- Code-generate type safe provider and llm models ([8a0be70](https://github.com/jcarver989/aether/commit/8a0be70c9114fd966f7f725d2e2172192e50fac8))
- Support agent model switching ([794741a](https://github.com/jcarver989/aether/commit/794741a3442dfa82e9220e720d6313b87b65f381))

### Refactoring

- Move oauth module into llm package ([396109f](https://github.com/jcarver989/aether/commit/396109f921e3c6e8ef1f99e06755278034a4714a))
- Cleanup dead code, extract tool call collector and cleanup deps ([a22edde](https://github.com/jcarver989/aether/commit/a22edde2c2c99981243096416ed788cc0813221e))
- Split MCPs into their own crates ([4333dc9](https://github.com/jcarver989/aether/commit/4333dc95632b5fc6ccb7427a18fcc5c8c46e8b90))
- Extract llm/ crate ([5a57735](https://github.com/jcarver989/aether/commit/5a5773542b847b87d9466703baf04523c08c0d4d))

### Feaet

- *(aether-cli)* Session logs and session restoration ([7b2649b](https://github.com/jcarver989/aether/commit/7b2649bfc0a1cbb54aeb8d2fa8fa5464a4e3ad77))
