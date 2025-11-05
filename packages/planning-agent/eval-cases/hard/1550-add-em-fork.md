# PR 1550: Add em.fork

## Issue Information
- **Issue**: #1546 - "Add em.fork"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1546
- **Issue Title**: Add em.fork

## PR Information  
- **PR**: #1550
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1550
- **PR Title**: feat: add em.fork
- **Author**: zgavin

## Commit Information
- **Commit SHA Prior to PR Merge**: `f4a1b3c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5`
- **Commit SHA After PR Merge**: `29cefdb6a482d6a235b2f058b966b23d429f4034`

## Complexity Rating
hard

## Why This Is a Good Eval Candidate

This is an excellent hard-level eval case because:

1. **Entity Manager Architecture**: Requires deep understanding of Joist's EntityManager architecture and state management.

2. **Entity Graph Duplication**: Must implement deep copying of loaded entity graphs while maintaining relationships and loaded states.

3. **Memory Management**: Forked EMs need careful memory management to avoid leaks while maintaining entity references.

4. **Transaction Isolation**: Forked EMs should provide isolation while potentially sharing some underlying connections/resources.

5. **Relation State Preservation**: The issue specifies maintaining `isLoaded` states across complex nested relations, requiring sophisticated state tracking.

6. **Preloading Cache Integration**: Must leverage existing preloading cache mechanisms to maintain loaded state consistency.

7. **Identity Map Management**: Need to handle entity identity across forked EMs while preventing unwanted mutations.

8. **Complex Performance Considerations**: Must balance memory usage, performance, and correctness in entity graph copying.

9. **Thread Safety**: Forked EMs need to handle concurrent access patterns safely.

10. **Deep ORM Knowledge**: Implementation requires understanding of entity lifecycle, caching, relation management, and query execution.

11. **Comprehensive Testing**: Needs tests for complex entity graphs, relation loading states, mutation isolation, and performance characteristics.

12. **Real-world Use Case**: This addresses practical needs for request-scoped entity managers in web applications and batch processing.

13. **Clear Technical Requirements**: The issue provides detailed specifications for expected behavior around entity state preservation and caching.