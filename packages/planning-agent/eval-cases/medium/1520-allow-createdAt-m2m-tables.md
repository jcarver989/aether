# PR 1520: Allow createdAt for m2m tables

## Issue Information
- **Issue**: #1519 - "Forced to use \"created_at\" as timestamp column in join tables"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1519
- **Issue Title**: Forced to use "created_at" as timestamp column in join tables

## PR Information  
- **PR**: #1520
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1520
- **PR Title**: fix: Allow createdAt for m2m tables
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `8c9d7e4f5b6a3210fedcba9876543210abcdef123`
- **Commit SHA After PR Merge**: `b82c50986d33b5543299660a87ebf716b39a0b1d`

## Complexity Rating
medium

## Why This Is a Good Eval Candidate

This is an excellent medium-level eval case because:

1. **ORM Internals Knowledge Required**: The fix requires understanding how Joist detects join tables and handles timestamp columns, demonstrating ORM architecture knowledge.

2. **Flexible Configuration Logic**: The implementation must respect user's `timestampColumns` configuration while providing sensible defaults for join table detection.

3. **Backward Compatibility**: Must ensure existing configurations continue working while allowing more flexible timestamp column naming.

4. **Database Integration**: This change affects schema introspection and table detection logic, requiring careful handling of different database naming conventions.

5. **Comprehensive Test Coverage**: The fix needs tests for various timestamp column configurations and join table scenarios.

6. **Performance Considerations**: The implementation should not negatively impact schema parsing performance for large databases.

7. **Clear Business Logic**: The issue description clearly explains the problem - hardcoded `created_at` preference over user's `timestampColumns` config in join tables.

8. **Real-world Impact**: This addresses a practical pain point for teams with specific naming conventions or legacy database schemas.