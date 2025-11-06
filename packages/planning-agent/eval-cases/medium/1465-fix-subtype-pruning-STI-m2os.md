# PR 1465: Fix subtype pruning for STI m2os

## Issue Information
- **Issue**: #1246 - "Subtype in STI m2o/o2m queries aren't fully pruned"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1246
- **Issue Title**: Subtype in STI m2o/o2m queries aren't fully pruned

## PR Information  
- **PR**: #1465
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1465
- **PR Title**: fix: Fix subtype pruning for STI m2os
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `4e3d2c1b5a9f8e7d6c5b4a3c2d1e0f9a8b7c6d5`
- **Commit SHA After PR Merge**: `c93dcd3ca039340b4c0eeba272664012e7e1fdc7`

## Complexity Rating
medium

## Why This Is a Good Eval Candidate

This is an excellent medium-level eval case because:

1. **Single Table Inheritance (STI) Knowledge**: Requires understanding of Joist's STI implementation and how subtype queries are generated.

2. **Query Optimization**: The fix involves optimizing WHERE clause generation to prune unnecessary subtype constraints when fields are undefined.

3. **GraphQL Integration**: The issue example shows GraphQL query patterns, requiring understanding of how GraphQL maps to database queries.

4. **Conditional Logic Implementation**: Must implement smart pruning that only adds subtype constraints when relevant fields are present.

5. **Type Safety**: The solution must maintain type safety while allowing dynamic query building based on input parameters.

6. **Edge Case Handling**: Needs to handle various input combinations and ensure correct SQL generation for each case.

7. **Performance Impact**: The optimization affects query performance, especially for large datasets with many subtypes.

8. **Testing Complexity**: Requires comprehensive tests covering different query shapes and input combinations to ensure pruning works correctly.

9. **Real-world Usage Pattern**: The issue describes a common pattern in GraphQL APIs where optional filters should not restrict query results unnecessarily.

10. **Clear Success Metrics**: Success can be measured by comparing generated SQL before and after the fix for various query inputs.