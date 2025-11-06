# PR 1406: Add includeSchema option to config

## Issue Information
- **Issue**: #1398 - "codegen: optionally define schemas to derive metadata from"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1398
- **Issue Title**: codegen: optionally define schemas to derive metadata from

## PR Information  
- **PR**: #1406
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1406
- **PR Title**: feat: add includeSchema option to config, with default of public
- **Author**: James Canning (brudil)

## Commit Information
- **Commit SHA Prior to PR Merge**: `a1b2c3d4e5f6789012345678901234567890abcd`
- **Commit SHA After PR Merge**: `2b95b88a12aa45b59f1304751e2b92e64c7f77f3`

## Complexity Rating
easy

## Why This Is a Good Eval Candidate

This is an excellent easy-level eval case because:

1. **Clear Configuration Feature**: The issue requests adding an optional `includeSchema` configuration option to support multiple PostgreSQL schemas, a well-defined enhancement request.

2. **Minimal, Targeted Changes**: The implementation involves adding configuration parsing and updating existing codegen logic to filter by specified schemas.

3. **Backward Compatible**: The feature is optional with a sensible default (`public`), ensuring existing functionality remains unchanged.

4. **Practical Multi-tenancy Use Case**: This addresses real-world needs for schema-based multi-tenancy, a common architectural pattern.

5. **Easy Testing**: The feature can be validated through integration tests that verify code generation respects schema filtering.

6. **Good Documentation Requirements**: The change requires updating both code comments and documentation, providing practice in comprehensive feature implementation.

7. **Clear Success Criteria**: Success is measurable through generated code output and configuration parsing behavior.