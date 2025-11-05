# PR 1482: Rename createOrUpdatePartial to upsert

## Issue Information
- **Issue**: #1478 - "Rename createOrUpdatePartial to upsert"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1478
- **Issue Title**: Rename createOrUpdatePartial to upsert

## PR Information  
- **PR**: #1482
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1482
- **PR Title**: feat: Rename createOrUpdatePartial to upsert
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `f9042e8a1ca5671f6b8c2c3d5b6f1a2b7c6d8e9f`
- **Commit SHA After PR Merge**: `10254c8e6c4867c4f55fd1be9e838e7a723c9175`

## Complexity Rating
easy

## Why This Is a Good Eval Candidate

This is an excellent easy-level eval case because:

1. **Clear Refactoring Task**: The issue explicitly requests renaming a method from `createOrUpdatePartial` to `upsert` for better idiomatic naming, which is a straightforward refactoring task.

2. **Well-Defined Scope**: The change involves renaming a specific method while maintaining the same functionality, making the expected behavior unambiguous.

3. **Comprehensive Impact**: This change touches multiple files across the codebase (method definition, calls, tests, documentation), providing good practice in systematic refactoring.

4. **Backward Compatibility Considerations**: The implementation must handle the transition smoothly, requiring careful deprecation handling and migration strategy.

5. **Easy Verification**: Success can be verified through automated tests that confirm the new method name works while maintaining identical functionality.

6. **Real-world Relevance**: API renaming is a common maintenance task in software development, especially for improving code clarity and adopting industry-standard terminology.