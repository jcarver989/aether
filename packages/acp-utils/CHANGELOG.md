# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-04-13

### Bug Fixes

- Docs ([bd31893](https://github.com/jcarver989/aether/commit/bd3189301357a20971271b776539462fedbd0d94))

### Features

- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(wisp)* Update provider login status when it changes ([be42db2](https://github.com/jcarver989/aether/commit/be42db27ae39eebb020846fa6ea86d7242721e3a))
- *(aether)* Support correct reasoning levels per model ([ef6e801](https://github.com/jcarver989/aether/commit/ef6e801258f10341f58febd1ce2dc687f134054d))
- *(llm)* Set cache control for OpenRouter ([003169d](https://github.com/jcarver989/aether/commit/003169dfb0c0be6b7e3aba68cda572bd5eeeb158))
- *(wisp)* Remove duplicated clear command as one came from wisp and another from aether-cli ([f556657](https://github.com/jcarver989/aether/commit/f556657ca1471150e019e7aa93e9de5a57d2e009))
- *(core)* Handle streaming tool updates ([bb2cc81](https://github.com/jcarver989/aether/commit/bb2cc81a37364b8662b3a2c51683909d3afcd928))
- *(mcp-servers)* Use similar crate to fix diffs ([859bf6d](https://github.com/jcarver989/aether/commit/859bf6d4e164e5456aa59087fcee2c49e1c10b89))
- Clippy warnings ([b6acd52](https://github.com/jcarver989/aether/commit/b6acd52772a328da37166439b893f4abbff9293b))
- Keyboard nav on file picker ([773b80d](https://github.com/jcarver989/aether/commit/773b80dfabb498b729f922e38b938dff1223afbf))

### Features

- *(aether)* Support prompts with images and audio ([f58754d](https://github.com/jcarver989/aether/commit/f58754d803f9458c34232756f2acc2ab59ff7183))
- *(aether-cli)* Session resume ([b8d225e](https://github.com/jcarver989/aether/commit/b8d225e88c13ddefedce58ce3e71ef27dc8e115b))
- Add settings file support to aether-cli ([6569f21](https://github.com/jcarver989/aether/commit/6569f216ead8a48245ebb7f6fd2e2f588791d46f))
- Prototype provider login from wisp ui ([a87bb7e](https://github.com/jcarver989/aether/commit/a87bb7e3b59a6ab7687f5166abf2d0fbaea88ff6))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- Clear slash command prototype ([7b23168](https://github.com/jcarver989/aether/commit/7b23168510ed867246656a764d33f14c0e2f58e1))
- Diffs in edit tool output ([fd4216c](https://github.com/jcarver989/aether/commit/fd4216c99b96bd13760cf01f8dc1a9d12ffcaa8a))
- Improved tooloutput for agents ([cc5920f](https://github.com/jcarver989/aether/commit/cc5920fdb2545229e707355edf2ef4cf1a156792))
- Support mcp elicitation through acp and support a form component in wisp ([1a96f39](https://github.com/jcarver989/aether/commit/1a96f399f88d1601f416ab526a00c4d1f50e9619))
- Nicer command picker and context in status line ([21a4f9c](https://github.com/jcarver989/aether/commit/21a4f9cfac219f37c3d27be3e1a7903a7e02ecce))
- Support agent model switching ([794741a](https://github.com/jcarver989/aether/commit/794741a3442dfa82e9220e720d6313b87b65f381))
- Soft line wrap tui ([5f4f9fe](https://github.com/jcarver989/aether/commit/5f4f9fe1c9cdc4c740c9faea66b4ba6d166313ec))

### Refactoring

- *(wisp)* Make wisp support acp Diff tool content ([2068e30](https://github.com/jcarver989/aether/commit/2068e306f9421182e65c4d03ce355f222929e83e))
- Handle cancel and config commands during in-flight ACP prompt ([06404b0](https://github.com/jcarver989/aether/commit/06404b086ff6797963925d56f7183b34bbf5a2a2))
- Split out acp-utils and remove cruft related to acp coding tools that aren't needed ([311b7a3](https://github.com/jcarver989/aether/commit/311b7a3d8ae094be45a8baa997fc77311fa9d10f))
