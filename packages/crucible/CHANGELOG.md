# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-04-13

### Features

- *(aether-cli)* Support MCP url elicitation ([09d6787](https://github.com/jcarver989/aether/commit/09d6787bbabe4c99987cd247eda8887f335a660a))
- *(aether-cli)* Support loading and merging multiple mcp.json files ([31821b3](https://github.com/jcarver989/aether/commit/31821b3809602d46e678876ca0d65cffc0762f10))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(core)* Handle streaming tool updates ([bb2cc81](https://github.com/jcarver989/aether/commit/bb2cc81a37364b8662b3a2c51683909d3afcd928))
- Clippy ([310bb21](https://github.com/jcarver989/aether/commit/310bb2171abc0c4eaf4fe7f0b089dfbacbbcc4a0))
- Clippy warnings ([b6acd52](https://github.com/jcarver989/aether/commit/b6acd52772a328da37166439b893f4abbff9293b))
- Match arm missing ([ffedd59](https://github.com/jcarver989/aether/commit/ffedd5998e86a8f7ac283ef67f620d3af5fef653))
- Auto-continue agent loop if llm prematurely gives up ([4c84844](https://github.com/jcarver989/aether/commit/4c84844752071cc5284517d8eead27b64dbd8829))
- Clippy ([60b9306](https://github.com/jcarver989/aether/commit/60b9306e65910812b72eba3168c35e4295cb1dc8))
- Missing catch ([8d4bc64](https://github.com/jcarver989/aether/commit/8d4bc640e434085d1b943ac40643e2f9d50fd5b8))
- Axum routes ([aa073d9](https://github.com/jcarver989/aether/commit/aa073d994d3e71fa85785d100d7cb420be4e043d))
- *(crucible)* Make control-c kill web server when running ([3eedf58](https://github.com/jcarver989/aether/commit/3eedf5839bfe7c224919ab0022ec56fe81b1db0e))
- *(crucible)* Log to jsonl trace and stdout show traces show up ([85ef1dc](https://github.com/jcarver989/aether/commit/85ef1dc4de94b7b03619eb2e0f095ea798469a81))
- *(crucible)* Make LLM as judge use json ([90a3de6](https://github.com/jcarver989/aether/commit/90a3de6440a8fb53c30edf54dbbdbe493e375a9b))

### Documentation

- *(crucible)* Align improvement proposals with best practices ([2a6386d](https://github.com/jcarver989/aether/commit/2a6386df33a67db254f9ee527d81ede63274483a))
- *(crucible)* Add comprehensive improvements feedback ([33ace83](https://github.com/jcarver989/aether/commit/33ace83e87a7586ff6c0c711a5d14a5fc200e010))

### Features

- *(aether)* Support prompts with images and audio ([f58754d](https://github.com/jcarver989/aether/commit/f58754d803f9458c34232756f2acc2ab59ff7183))
- Mcp oauth works in wisp, tool proxy prototype ([bc21bd2](https://github.com/jcarver989/aether/commit/bc21bd2fabbfc095215e6457a7f30327dcf06cf8))
- MCP to proxy other MCP tools ([6e0989b](https://github.com/jcarver989/aether/commit/6e0989b503a2c6f81b0d997db5a41c9a5538ddeb))
- Clear slash command prototype ([7b23168](https://github.com/jcarver989/aether/commit/7b23168510ed867246656a764d33f14c0e2f58e1))
- Breakout aether-cli ([80d65a3](https://github.com/jcarver989/aether/commit/80d65a3efd4eba3e560dfa7cf4d7731aeb5ba363))
- Support agent model switching ([794741a](https://github.com/jcarver989/aether/commit/794741a3442dfa82e9220e720d6313b87b65f381))
- Add MCP instructions to system prompt ([7431914](https://github.com/jcarver989/aether/commit/74319145bb3250b608794049e9580955c141239b))
- Compaction ([0e1e13f](https://github.com/jcarver989/aether/commit/0e1e13fc889a67ce9011e101716b07907d43cbba))
- *(crucible)* Rudimentary git diffs ([35fdde1](https://github.com/jcarver989/aether/commit/35fdde117514590e6e977eed575d202d1fc08ee3))
- *(crucible)* Add setup hook so you can run commands before agent runs, e.g. to install npm deps ([f0c5a26](https://github.com/jcarver989/aether/commit/f0c5a260134cb28aec526132483a5f03be76e593))
- *(crucible)* Add hook to run before evals so we can use Claude Code after planning agent ([34df470](https://github.com/jcarver989/aether/commit/34df470e8f782874530429f2435bcee1e5d5fdf8))
- *(crucible)* Inject git diff for git evals ([40aa236](https://github.com/jcarver989/aether/commit/40aa236e17efa497a1480b14c33f4e3773b34691))
- *(aether)* ToolProgress notifications emited from AgentMessage ([b2dff1b](https://github.com/jcarver989/aether/commit/b2dff1b0e459985986042190c3aca89720ddb8c3))
- *(crucible)* Run http server for eval results ([ec1c8a1](https://github.com/jcarver989/aether/commit/ec1c8a1907926767124ec2759fd0bf1fac2c5a6f))
- *(crucible)* HTML report + ui for browsing traces ([347ee3d](https://github.com/jcarver989/aether/commit/347ee3d5197e5d80f819c58841872e4317076119))
- *(crucible)* Add crucible package for evals ([52aaee5](https://github.com/jcarver989/aether/commit/52aaee55bf55d0deb04235243afb69e2e4e4f0ce))

### Refactoring

- Cleanup AgentBuilder system prompt methods to just be system_prompt() ([6687555](https://github.com/jcarver989/aether/commit/6687555ea55a54ea9343d24d382077a5fe232b3d))
- Absorb agent-events into aether package ([472f6c5](https://github.com/jcarver989/aether/commit/472f6c55b77c51aa8ae4ce30fea9f7f3c223bca6))
- Split out acp-utils and remove cruft related to acp coding tools that aren't needed ([311b7a3](https://github.com/jcarver989/aether/commit/311b7a3d8ae094be45a8baa997fc77311fa9d10f))
- Remove re-xports and import directly ([940e2d6](https://github.com/jcarver989/aether/commit/940e2d6743c3a7cd34094a8422225ed9f086cd2c))
- Rename agent/ module to core/ ([f204272](https://github.com/jcarver989/aether/commit/f204272c11d66c6da7802d4f83f6c7b465e3b133))
- Split MCPs into their own crates ([4333dc9](https://github.com/jcarver989/aether/commit/4333dc95632b5fc6ccb7427a18fcc5c8c46e8b90))
- Extract llm/ crate ([5a57735](https://github.com/jcarver989/aether/commit/5a5773542b847b87d9466703baf04523c08c0d4d))
- Migrate Hook trait to async-trait crate ([1baa71a](https://github.com/jcarver989/aether/commit/1baa71a7df2007f04ec975a36312b3dad5a5c645))
- *(crucible)* Move eval related files into evals/ ([7cb6006](https://github.com/jcarver989/aether/commit/7cb6006c302895e1700a3db80f670bb33f3a6dbd))
- *(crucible)* Add AgentRunner trait to decouple from aether agents only ([a42d4ce](https://github.com/jcarver989/aether/commit/a42d4ce36720ac5d1669fdd5e103774103db1647))
- *(crucible)* Extract eval_config and eval_runner from lib.rs ([9b592c7](https://github.com/jcarver989/aether/commit/9b592c7a2c26519f78a6a7bfd6c116651293af06))
- Track eval state started, running, completed ([fd279f1](https://github.com/jcarver989/aether/commit/fd279f1197ac8d6ebc9de5f4a8340ae2f212fbac))
- *(crucible)* Refactor to use axum server ([4a32439](https://github.com/jcarver989/aether/commit/4a32439eadd1d1e62491e521663be1adfdb036d9))
- *(crucible)* Make EvalMetric enum wrap discrete typs so we can extract json schemas from those types ([2ffcaa6](https://github.com/jcarver989/aether/commit/2ffcaa645679e25a6e4d1a3a1f3f72af9c49bfd1))
- *(crucible)* Switch lib/eval.rs to using a programatic API ([6700aa8](https://github.com/jcarver989/aether/commit/6700aa8ff0b21bd61ceb281b075455c57005035d))
