# Crucible Improvements

This directory contains detailed proposals for improving the Crucible eval framework. Each improvement includes problem description, proposed solution, implementation details, and code examples.

## Priority Overview

### 🔴 P0 - Critical (Blockers for Production)

These improvements are **essential** for production use and should be implemented first:

1. **[Timeouts](./01-timeouts.md)** - Prevent runaway agents from blocking eval runs indefinitely
2. **[LLM Judge JSON Parsing](./02-llm-judge-json-parsing.md)** - Fix fragile JSON parsing that breaks with markdown wrapping
3. **[Cost Tracking](./03-cost-tracking.md)** - Track and report LLM API costs for budgeting

**Estimated Total Effort:** 1-2 weeks
**Total LOC:** ~390

### 🟡 P1 - High Priority (Major Functionality Gaps)

These improvements add significant functionality and developer experience improvements:

4. **[File Matches Strategies](./04-file-matches-strategies.md)** - Support regex, LLM judges, and format validation for file content
5. **[Parallel Assertions](./05-parallel-assertions.md)** - Run independent assertions concurrently for faster eval runs
6. **[Tool Argument Matching](./06-tool-argument-matching.md)** - Flexible argument matching (partial, exact, predicates)

**Estimated Total Effort:** 1 week
**Total LOC:** ~240

### 🟢 P2 - Medium Priority (Quality of Life)

These improvements enhance testing workflows and system reliability:

7. **[Regression Testing](./07-regression-testing.md)** - Compare eval runs to detect regressions and track improvements
8. **[Memory Limits](./08-memory-limits.md)** - Prevent OOM by bounding message buffer size
9. **[Test Coverage](./09-test-coverage.md)** - Comprehensive unit and integration tests

**Estimated Total Effort:** 2-3 weeks
**Total LOC:** ~1350+

## Implementation Roadmap

### Week 1: Quick Wins (P0)

**Goal:** Fix critical blocking issues

- Day 1-2: LLM Judge JSON parsing fix (#2) - Unblocks reliable LLM judges
- Day 3-4: Timeout support (#1) - Prevents runaway evals
- Day 5: Cost tracking foundations (#3)

**Deliverable:** Crucible can run reliably without hanging or failing on markdown-wrapped JSON

### Week 2: Foundation (P0 + P1)

**Goal:** Complete P0 items and start high-value P1 features

- Day 1-2: Complete cost tracking (#3) with display and aggregation
- Day 3: File matching strategies (#4) - Regex and format validation
- Day 4: Better tool argument matching (#6)
- Day 5: Parallel assertions (#5)

**Deliverable:** Crucible has robust core features with good performance

### Week 3-4: Polish (P2)

**Goal:** Add professional features for team/enterprise use

- Week 3: Regression testing support (#7)
- Week 4: Memory limits (#8) and test coverage (#9)

**Deliverable:** Production-ready eval framework

## Implementation Notes

### Dependency Order

Some improvements depend on or benefit from others:

- **Cost Tracking (#3)** benefits from **Timeouts (#1)** to prevent unbounded costs
- **Regression Testing (#7)** requires **Cost Tracking (#3)** for complete comparison
- **Test Coverage (#9)** should be done alongside each feature implementation

### Breaking Changes

Most improvements are backward compatible, but note:

- **File Matches Strategies (#4)**: Changes `FileMatches` assertion signature
  - Migration: `content: String` → `strategy: FileMatchStrategy::Contains(content)`
  - Can provide `file_matches()` wrapper for backward compatibility

- **Tool Argument Matching (#6)**: Changes `ToolCall` assertion signature
  - Migration: `arguments: Option<Value>` → `arguments: Option<ArgumentMatchStrategy>`
  - Can provide wrapper methods for backward compatibility

### Testing Strategy

Each improvement should include:

1. **Unit tests** - Test the feature in isolation
2. **Integration tests** - Test with `FakeAgentRunner` and real storage
3. **Documentation** - Update README and examples
4. **Example usage** - Add to `examples/` directory

## Quick Reference

| # | Improvement | Priority | Impact | Effort | LOC |
|---|-------------|----------|--------|--------|-----|
| 1 | Timeouts | 🔴 P0 | Critical | Medium | 150 |
| 2 | LLM Judge JSON | 🔴 P0 | High | Low | 40 |
| 3 | Cost Tracking | 🔴 P0 | High | Medium | 200 |
| 4 | File Match Strategies | 🟡 P1 | High | Low | 100 |
| 5 | Parallel Assertions | 🟡 P1 | Medium | Low | 80 |
| 6 | Tool Arg Matching | 🟡 P1 | Medium | Low | 60 |
| 7 | Regression Testing | 🟢 P2 | Medium | Medium | 300 |
| 8 | Memory Limits | 🟢 P2 | Low-Med | Low | 50 |
| 9 | Test Coverage | 🟢 P2 | High | High | 1000+ |

## Contributing

When implementing an improvement:

1. Read the full proposal document
2. Create a feature branch: `git checkout -b improvement/01-timeouts`
3. Write tests first (TDD approach preferred)
4. Implement the feature following the proposal
5. Update relevant documentation
6. Create PR with reference to the improvement doc

## Questions?

For questions or discussion about these improvements:

- Open an issue in the main repository
- Reference the specific improvement number
- Tag with `enhancement` label
