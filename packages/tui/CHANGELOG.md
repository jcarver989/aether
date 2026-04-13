# Changelog

All notable changes to this project will be documented in this file.

## [0.1.4] - 2026-04-13

### Bug Fixes

- *(tui)* Cleanup terminal output when exiting the tui loop ([af0743b](https://github.com/jcarver989/aether/commit/af0743b74b66e04278f2c186ee5d4f5611f1d26d))
- *(tui)* Gutter in split diff with wrong bg color ([965c386](https://github.com/jcarver989/aether/commit/965c38650676915958c8381c15b7388b07e4318d))
- *(docs)* Doc tests ([f63df28](https://github.com/jcarver989/aether/commit/f63df28579875bb2abf726eca1898190f257acdf))
- *(wisp)* Diff wrapping ([76bbbe1](https://github.com/jcarver989/aether/commit/76bbbe1b6615da79a57ffe89908e6fc2c434d24f))
- *(wisp)* Diff line style bleeding into the next line's gutter when wrapped ([a77ad1b](https://github.com/jcarver989/aether/commit/a77ad1bc3902191c14be60fdaafa83985e7cb80e))
- *(wisp)* Diff style bleeding into gutter on line wrap ([efb2a28](https://github.com/jcarver989/aether/commit/efb2a28c5f9dacd9b11c43acd64e0d81a7fbaa4a))
- *(wisp)* Split diff lines wrapping into another pane ([f87460e](https://github.com/jcarver989/aether/commit/f87460e5ca31e6d47e53a79357681e299ed97dc9))
- *(tui)* Softwrap lines at whitespace, not word boundaries ([9a6bdd3](https://github.com/jcarver989/aether/commit/9a6bdd350182856ab9cba540359ac05cda348e25))

### Features

- *(tui)* Add Stepper component ([9400157](https://github.com/jcarver989/aether/commit/9400157b40dcc393092cef7565996a5717c484ea))
- *(tui)* Add TerminalRuntime as main entrypoint ([ca75cab](https://github.com/jcarver989/aether/commit/ca75cab1fa334bf42830909f6aafdd743bc19ec0))
- *(tui)* Add BorderedTextField ([c865977](https://github.com/jcarver989/aether/commit/c865977c16947e4b3877b0ffbed45930295f2492))
- *(wip)* Double cntrl-c to exit ([09dda29](https://github.com/jcarver989/aether/commit/09dda29f41c85c63a02fd94883895236d3f94d9d))
- *(tui)* Add Frame::splice to join frames together ([6191722](https://github.com/jcarver989/aether/commit/61917228fc2809463e7a25797ada9f45f150d2f6))

### Release

- V0.1.3 ([c024669](https://github.com/jcarver989/aether/commit/c024669671ec935afedd8581b165c20676d376e8))
## [0.1.2] - 2026-04-05

### Bug Fixes

- *(workspace)* All clippy warnings ([9698cf8](https://github.com/jcarver989/aether/commit/9698cf8a674c66a7408553450bccbdf9696e6160))
- *(workspace)* Clippy ([5da71cb](https://github.com/jcarver989/aether/commit/5da71cbbbc2bb275278b8d2687add5c3c136fd02))
- Clippy ([3dcc9d2](https://github.com/jcarver989/aether/commit/3dcc9d2b7788e45e6ddaae3eb4138f954174e261))
- Split_panel wrapping ([4ac814d](https://github.com/jcarver989/aether/commit/4ac814d1fc0cb6f4620946322c27a76a0efd30fb))
- *(tui)* Show unified diff instead of split-diff if all lines are additions ([6e41507](https://github.com/jcarver989/aether/commit/6e41507cb95522f56763ec031af770614d7af302))
- *(tui)* Rendering on resize now clears entire terminal and re-renders ([62a0d2d](https://github.com/jcarver989/aether/commit/62a0d2db0869422e26400c6492e17aae8d6b571e))
- *(tui)* Theme compositing colors with alpha values ([7e43b90](https://github.com/jcarver989/aether/commit/7e43b9035d686e20315fc162bcd14d9ebca59d3a))
- *(tui)* Show edited region in diff preview, not beginning of file ([1b112d7](https://github.com/jcarver989/aether/commit/1b112d7655e8fe845ca7a57a29880b05ab892b7c))
- Clippy ([80c1892](https://github.com/jcarver989/aether/commit/80c18924d307357e88e745cc0ca9e75647363c79))
- *(wisp)* "/clear" ([3b985bf](https://github.com/jcarver989/aether/commit/3b985bf22f4f9d3fb7bad393881b74adf61fdbe1))
- *(tui)* Don't clone synnect theme on every render ([087c803](https://github.com/jcarver989/aether/commit/087c80302f5dad5bb13b29181e21a773b6daf947))

### Features

- *(tui)* Add SplitPanel ([c1a1d0a](https://github.com/jcarver989/aether/commit/c1a1d0acb08aa8eb0be6c905c7cc18f0767fc40e))
- *(tui)* Add gallery to examples ([ad90b04](https://github.com/jcarver989/aether/commit/ad90b04102c0fe770fa2b4377da73b3370a2d81c))
- *(wisp)* Git diff split view ([be59521](https://github.com/jcarver989/aether/commit/be59521655db47d3ad45e2a4da2f0298369a3482))
- *(tui)* Syntax highlighting for additional languags, e.g. typescript ([d48fe0b](https://github.com/jcarver989/aether/commit/d48fe0b166847ff05036030d647d45216595fce2))
- *(tui)* Prototype diff split-view ([38b11fe](https://github.com/jcarver989/aether/commit/38b11feffaf2b453e04a696e1f086ceaba662151))
- *(wisp)* Enable mouse scrolling in settings ([c045c98](https://github.com/jcarver989/aether/commit/c045c983e9d2a8c82ba17da04f566d078b51c56b))
- *(wisp)* Tabbed-survey form and better input keyboard shortcut ([d3fd83c](https://github.com/jcarver989/aether/commit/d3fd83cd49135f6505963f4de3041123d49f4532))
- *(wisp)* Prototype git diff view, and fix resize bug ([9de271e](https://github.com/jcarver989/aether/commit/9de271ef21d2296df91a6293326da4f6e3173ade))
- *(tui)* Make render method on Component &self, not &mut self ([e7cbb9f](https://github.com/jcarver989/aether/commit/e7cbb9fa149719a28a7b9c06517022f8f826ee15))
- *(workspace)* Break up packages, add a funky logo for kicks ([f7c828d](https://github.com/jcarver989/aether/commit/f7c828ded51e0b7db74328e1fa75b5573c85a027))
- *(core)* Working coding agent example with nice-ish formatting ([44b258d](https://github.com/jcarver989/aether/commit/44b258dfae223097b09f3e8c0394cb677d9d1bb6))
- *(core)* Working example of InMemory transport with actual agent tools ([32d7b86](https://github.com/jcarver989/aether/commit/32d7b86c2c8ffe43c556a8731200e6dd962f6e97))

### Refactoring

- *(wisp)* Git diff view ([ab31841](https://github.com/jcarver989/aether/commit/ab318412da136927f8a344e984d3de342764260c))
- *(tui)* Clean up render method ([305c25b](https://github.com/jcarver989/aether/commit/305c25bd06fb14cde86033e1af40af801e912da8))
- *(tui)* Move diff() to VisualFrame to cleanup Renderer ([01bd6b0](https://github.com/jcarver989/aether/commit/01bd6b00e3a6a52f51271d2960a678329af23f07))
- *(tui)* Make COmponent::render tae &mut self so components can cache internally during render ([53ceba8](https://github.com/jcarver989/aether/commit/53ceba892ef128c3c205d620a66e2b513b324271))
- *(tui)* Move RendererCommand and apply_command from wisp/ -> tui/ ([d450265](https://github.com/jcarver989/aether/commit/d45026505f57b98b213c071cf5e6dc0b6d8c205a))
- *(tui)* Make on_event  async ([9b6d808](https://github.com/jcarver989/aether/commit/9b6d808ad30924ed7663c39d0d8737011b9e3674))
- *(wisp)* Move unit tests to integration tests and continue refactor ([77cdc10](https://github.com/jcarver989/aether/commit/77cdc10b4db95f46b9961c3a432279eed9e51a9f))
- *(wisp)* Split tool components up and extract fuzzy matcher ([f8b563b](https://github.com/jcarver989/aether/commit/f8b563b73ddd57626445a7d12438a390f40a8f55))
- *(wisp)* Continue to refactor monolithic components into smaller components ([4c1e243](https://github.com/jcarver989/aether/commit/4c1e243196fd171d386209f68dd80f714d483dde))
- Start to make tui/ more of lib than framework ([2d57014](https://github.com/jcarver989/aether/commit/2d5701495c42f3b2e418729a653b0895e276557e))
- *(tui)* Clean up rendering ([e83578c](https://github.com/jcarver989/aether/commit/e83578cae32bf1c6e87697aaad2ced99cd26f4da))
- *(tui)* Split out TerminalScreen ([67a657a](https://github.com/jcarver989/aether/commit/67a657a1ae5890e2a7d6bec832b67987938561fc))
- *(tui)* Simplify by removing uneeded Command struct ([38f0d41](https://github.com/jcarver989/aether/commit/38f0d4126945281b204783547e1d987c85c6759e))
- *(tui)* Continue refactor ([f9c2f23](https://github.com/jcarver989/aether/commit/f9c2f238e92cf0982c80757b7caf3c26407927ea))
- *(tui)* Begin to clean up the tui crate api ([8e6b171](https://github.com/jcarver989/aether/commit/8e6b171ca579183a9955158d42c49070f324497c))
- *(tui)* Wip -- more elm like architecture ([7be6246](https://github.com/jcarver989/aether/commit/7be624655bee87ebc76feae98a854ef23c0356db))
- *(tui)* Move to props based rendering ([6737760](https://github.com/jcarver989/aether/commit/67377600217ed38ecd0eec43fcf6c672d81effe3))
- *(wisp)* Git diff cleanups ([49c069c](https://github.com/jcarver989/aether/commit/49c069cf134cbac2582481912f6d6d068e21269b))
- *(tui)* Cleanups ([23a0955](https://github.com/jcarver989/aether/commit/23a095553a0eaeccfa9b003160a1a7ee626cd165))
- *(tui)* App trait ([df7e511](https://github.com/jcarver989/aether/commit/df7e5112f9dd96f540f1b5fcae38bc8d430a442b))
- *(tui)* Add SyntaxHighlighter struct ([38d4f79](https://github.com/jcarver989/aether/commit/38d4f790cc3e8f1b4862e4c37851fd0bb43d3627))
- *(tui)* Renames and split responsibilities better ([14a9907](https://github.com/jcarver989/aether/commit/14a9907a803509515291b60e4cc2d13ab12bdc9b))
- *(tui)* Extract tui crate ([0214efe](https://github.com/jcarver989/aether/commit/0214efe7e99b72f85485df5096fe22798cac53e0))
