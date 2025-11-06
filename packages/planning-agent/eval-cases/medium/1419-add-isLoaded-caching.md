# PR 1419: Add isLoaded caching

## Issue Information
- **Issue**: #1166 - "Optimize isLoaded checks"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1166
- **Issue Title**: Optimize isLoaded checks

## PR Information  
- **PR**: #1419
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1419
- **PR Title**: fix: Add isLoaded caching
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `7d5e9f3a2b1c4d8e9f6a5b3c2d1e0f9a8b7c6d5`
- **Commit SHA After PR Merge**: `cc101b9926abfbb11362db5ae630877eeea2193e`

## Complexity Rating
medium

## Why This Is a Good Eval Candidate

This is an excellent medium-level eval case because:

1. **Performance Optimization Challenge**: The issue requires implementing a caching system to optimize repeated `isLoaded` checks, demonstrating understanding of performance bottlenecks.

2. **State Management Complexity**: Must implement cache invalidation logic when entity graphs are mutated, requiring careful state tracking.

3. **Trade-off Analysis**: The detailed issue description discusses different invalidation strategies (fine-grained vs coarse-grained), requiring architectural decision-making.

4. **Graph Theory Understanding**: The solution needs to understand entity relationships and dependency tracking for proper cache invalidation.

5. **Memory vs CPU Trade-offs**: Must balance memory usage of caching against CPU performance gains, showing optimization maturity.

6. **Comprehensive Testing**: Requires tests for cache hit/miss scenarios, invalidation correctness, and performance benchmarks.

7. **Complex Composite Relations**: The issue mentions handling complex reactive fields and composite relations, requiring deep ORM knowledge.

8. **Instrumentation Requirements**: Implementation may need to instrument existing mutation methods (`m2o.set`, `m2m.add/remove`) for cache invalidation.

9. **Real-world Performance Impact**: This addresses actual performance issues encountered in production with complex entity graphs.

10. **Clear Problem Statement**: The issue provides detailed explanation of the performance problem and potential solutions, making requirements clear.