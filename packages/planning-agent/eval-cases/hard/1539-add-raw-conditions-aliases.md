# PR 1539: Add raw conditions to aliases

## Issue Information
- **Issue**: #699 - "Support raw conditions on find queries"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/699
- **Issue Title**: Support raw conditions on find queries

## PR Information  
- **PR**: #1539
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1539
- **PR Title**: feat: Add raw conditions to aliases
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `9d2e4c1f5b6a3789abcdef0123456789012345678`
- **Commit SHA After PR Merge**: `bf757debc0622cbfa4c777fd0c5b40acd83cb954`

## Complexity Rating
hard

## Why This Is a Good Eval Candidate

This is an excellent hard-level eval case because:

1. **Advanced Query Building**: Requires implementing raw SQL condition support within Joist's type-safe query builder, a sophisticated ORM feature.

2. **Type System Integration**: Must maintain type safety while allowing raw SQL, requiring careful TypeScript type design.

3. **SQL Injection Prevention**: Raw conditions need proper parameterization and escaping to prevent security vulnerabilities.

4. **Multiple Query APIs**: The issue examples show both direct conditions and alias-based approaches, requiring flexible API design.

5. **PostgreSQL-Specific Features**: The examples include PostgreSQL-specific features like `@@` and `plainto_tsquery`, requiring database dialect awareness.

6. **Query Compilation Complexity**: Raw conditions must integrate with Joist's existing query compilation and optimization pipeline.

7. **Testing Challenges**: Requires testing complex SQL scenarios, parameter binding, and edge cases for raw SQL injection prevention.

8. **Alias Management**: The alias-based approach requires understanding of table aliases and SQL scope management.

9. **Performance Considerations**: Raw conditions should not bypass query optimizations or caching mechanisms.

10. **Documentation and Safety**: The feature needs clear documentation about safe usage patterns and security implications.

11. **Real-world Advanced Use Cases**: Addresses legitimate needs for complex database queries that go beyond standard ORM capabilities.

12. **Backward Compatibility**: Must ensure existing query functionality remains unchanged while adding raw capabilities.