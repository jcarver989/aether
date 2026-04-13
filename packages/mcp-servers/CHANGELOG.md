# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-04-13

### Bug Fixes

- Test ([0327c73](https://github.com/jcarver989/aether/commit/0327c73859fa473f8e516b873ecf888395dc9854))

### Features

- *(aether-cli)* Support shell command expansion in prompt files ([f3ea6f5](https://github.com/jcarver989/aether/commit/f3ea6f56142f35354f585082edfd3196a84a6547))
- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
- *(aether-cli)* Support loading and merging multiple mcp.json files ([31821b3](https://github.com/jcarver989/aether/commit/31821b3809602d46e678876ca0d65cffc0762f10))
- *(mcp-servers)* Skills server now supports multiple skill dirs ([3804e1b](https://github.com/jcarver989/aether/commit/3804e1bb694ded010a24ed520457bffd9afaeae3))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(llm)* Make provider factory async to avoid block call in Bedrock ([44e727a](https://github.com/jcarver989/aether/commit/44e727afd19313f296071cc7862ed993d5b1af6e))
- *(mcp-servers)* Flaky test ([a9a97a5](https://github.com/jcarver989/aether/commit/a9a97a5ea4772d106e56bfabecd67e496aec61df))
- *(mcp-servers)* Compiler error ([40a4fa7](https://github.com/jcarver989/aether/commit/40a4fa7245171312cf2c02445f90708a9b07b8b1))
- *(mcp-servers)* Tweak grep tool schema and description to avoid agent failing to use it ([2f5ce6f](https://github.com/jcarver989/aether/commit/2f5ce6ff0ef82dd75fe8d127ad6ae09c40a3f273))
- Tests ([65f57ff](https://github.com/jcarver989/aether/commit/65f57ff0496efcd902fc7b44e0618c9e58316cb6))
- *(lspd)* Cannonicalize root path to avoid duplicated lsp processes ([53a53d3](https://github.com/jcarver989/aether/commit/53a53d3e7abbe9321d91918404b9e9b8ff47511e))
- *(mcp-servers)* Make list_files tool interpret path: "" as list cwd to help agents avoid failures ([48a6710](https://github.com/jcarver989/aether/commit/48a67105ecb80a865c71772ed5f6465949983360))
- *(mcp-servers)* Test ([af97371](https://github.com/jcarver989/aether/commit/af97371767ebdff904aa4167c0e59c0793cf6ee8))
- *(mcp-servers)* Broken test ([22f42cd](https://github.com/jcarver989/aether/commit/22f42cddb44767b5bc222ebe522acf4b8082dbcd))
- *(aether-lspd)* E2E tests not cleaning up daemons and causing ooms on linux ([5c94ebd](https://github.com/jcarver989/aether/commit/5c94ebdf6d2ccb257de43982a37af2ad84c9fcc6))
- *(mcp-servers)* Use similar crate to fix diffs ([859bf6d](https://github.com/jcarver989/aether/commit/859bf6d4e164e5456aa59087fcee2c49e1c10b89))
- Allow parent_id empty string to be interpreted as None ([941a231](https://github.com/jcarver989/aether/commit/941a231f6dcca614bb74619f618352131321e0d8))
- Broken agent on API errors and show line numbers on file tools ([a4aa921](https://github.com/jcarver989/aether/commit/a4aa9214f512dd52f2de5f4805013f9692bc16d3))

### Features

- *(mcp-servers)* Add permission-mode setting to coding-server ([1ff1807](https://github.com/jcarver989/aether/commit/1ff18078807261279517cf013c84209272ec2e03))
- *(aether-cli)* Support filtering tools for sub-agents in settings file ([56a13b4](https://github.com/jcarver989/aether/commit/56a13b4189bc6b8218c2fa6dca845e452af7dcdd))
- *(mcp-servers)* Split skills from notes ([96e7e3e](https://github.com/jcarver989/aether/commit/96e7e3eca1cf1a88cbce06076746338022b8d0a6))
- *(mcp-servers)* Support loading auxilary files in skill directories ([b3235bc](https://github.com/jcarver989/aether/commit/b3235bcd0771c36684f642a5ba910a679865b600))
- *(mcp-servers)* Unify slash commands, skills and rules ([17737e4](https://github.com/jcarver989/aether/commit/17737e46f0fa10fe26f79a893f877ada529fcb33))
- *(aether)* New settings.json format for sub-agents ([9371004](https://github.com/jcarver989/aether/commit/9371004c1685658fcc0e331762853a2936a96f11))
- *(aether-cli)* Customizable, composible prompts via globs ([8b858b6](https://github.com/jcarver989/aether/commit/8b858b60e056d8851182e771aac9c45fc118472d))
- *(mcp-servers)* Prototype rename field ([824a11d](https://github.com/jcarver989/aether/commit/824a11d7b06cb106f6fa90ff5148b6befa21cc6e))
- *(mcp-servers)* Show tool meta display diff for write_file tool ([10dbc25](https://github.com/jcarver989/aether/commit/10dbc2563d085d5db9d3093853282c3cbd597a58))
- *(mcp-servers)* Show tool progress meta display ([7cd250c](https://github.com/jcarver989/aether/commit/7cd250c9f9edf283eaf980004e3a8b4ba0402035))
- Prototype plan view ([1303209](https://github.com/jcarver989/aether/commit/130320904fc62871fac3caf70f2a30ba7dd60f21))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- MCP proxy for tools ([983a29a](https://github.com/jcarver989/aether/commit/983a29a74e7dbabe1e23f4d6854e50b9eb8d528f))
- MCP to proxy other MCP tools ([6e0989b](https://github.com/jcarver989/aether/commit/6e0989b503a2c6f81b0d997db5a41c9a5538ddeb))
- Skill MCP server allows agent to author skill entries ([38df10d](https://github.com/jcarver989/aether/commit/38df10d3711189f06b9d30940289ba60ad7113a6))
- Clear slash command prototype ([7b23168](https://github.com/jcarver989/aether/commit/7b23168510ed867246656a764d33f14c0e2f58e1))
- Diffs in edit tool output ([fd4216c](https://github.com/jcarver989/aether/commit/fd4216c99b96bd13760cf01f8dc1a9d12ffcaa8a))
- Improved tooloutput for agents ([cc5920f](https://github.com/jcarver989/aether/commit/cc5920fdb2545229e707355edf2ef4cf1a156792))
- Breakout aether-cli ([80d65a3](https://github.com/jcarver989/aether/commit/80d65a3efd4eba3e560dfa7cf4d7731aeb5ba363))
- Add survey mcp server to demo elicitation ([4f18dc1](https://github.com/jcarver989/aether/commit/4f18dc1ffbe5148e611107cae9dedbce4604127c))

### Refactoring

- *(lspd)* Start to cleanup the lspd code by splitting up modules ([f9d0659](https://github.com/jcarver989/aether/commit/f9d0659a0c0d42a93a5d8fe751326ed237a16892))
- *(aether-project)* Start to clean up catalog ([a426292](https://github.com/jcarver989/aether/commit/a42629213bd928168da0cc67f3bed480bfcb8a13))
- *(wisp)* Make wisp support acp Diff tool content ([2068e30](https://github.com/jcarver989/aether/commit/2068e306f9421182e65c4d03ce355f222929e83e))
- *(mcp-servers)* LSP tool error checking schema is now clearer for workspace vs file mode ([34d1c40](https://github.com/jcarver989/aether/commit/34d1c404db8fd1a410813cf9c0e3dcbcea9fbbd4))
- *(wisp)* Git diff cleanups ([49c069c](https://github.com/jcarver989/aether/commit/49c069cf134cbac2582481912f6d6d068e21269b))
- *(mcp-servers)* Make tool prompts more concise and give consistent format ([6dadd50](https://github.com/jcarver989/aether/commit/6dadd50dc3fb53555df72398fc54357ea2923cf3))
- Skills, make tools take notes ([9a0a8c3](https://github.com/jcarver989/aether/commit/9a0a8c3aef3826a508c78134609ab49d6bc28fe9))
- Checkpoint working lspd file watcher changes ([ce17ee0](https://github.com/jcarver989/aether/commit/ce17ee0986395302836379c4925d697854991642))
- Lsp tools ([6916652](https://github.com/jcarver989/aether/commit/691665220e0baf05fea4ce8e7ecb2e3f85349a65))
- Lsp coding tools are now split from the coding mcp server with stdio support for other agents ([bfa9e94](https://github.com/jcarver989/aether/commit/bfa9e94a9ed24cf1d993ba2010a21fa7e05e7a44))
- Consolidate mcp servers into single crate again and feature flag this time ([064ef4c](https://github.com/jcarver989/aether/commit/064ef4c9ac280fc8e04a169ed742d1e2b860faf6))
