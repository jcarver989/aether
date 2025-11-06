# PR 1652: Make em.create ids stable in toString

## Issue Information
- **Issue**: #1625 - "Make entity.toString stable for new entities"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1625
- **Issue Title**: Make entity.toString stable for new entities

## PR Information  
- **PR**: #1652
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1652
- **PR Title**: feat: Make em.create ids stable in toString
- **Author**: Stephen Haberman (stephenh)

## Commit Information
- **Commit SHA Prior to PR Merge**: `c7b7a3acae44310e7ba9cb0c258acd86bdf0ac20`
- **Commit SHA After PR Merge**: `9408ffaf0e5743e6707f4738f68fb6baaaada7da`

## Complexity Rating
hard

## Why This Is a Good Eval Candidate

This is an excellent hard-level eval case because:

1. **Entity Lifecycle Management**: Requires deep understanding of Joist's entity creation, identity assignment, and lifecycle management.

2. **Performance Optimization**: The issue mentions an O(n²) performance problem with `entity.toString()`, requiring algorithmic improvements.

3. **State Consistency**: Must maintain entity state consistency across flush operations and identity assignment.

4. **Counter Management**: Implementation requires persistent counters per entity meta that survive entity manager flushes.

5. **Debugging Enhancement**: The change improves debugging capabilities by making entity identification stable and predictable.

6. **Multiple Representation Formats**: Must handle both temporary IDs (`#1`) and permanent IDs (`:1`) and their combination (`#1:1`).

7. **Memory Management**: Counter persistence requires careful memory management to prevent leaks while maintaining uniqueness.

8. **Entity Manager Integration**: Changes affect core entity manager behavior and entity initialization logic.

9. **Testing Complexity**: Requires tests for entity creation, flush operations, toString performance, and counter uniqueness.

10. **Backward Compatibility**: The new format (`#1:1`) is different from existing behavior, requiring careful migration considerations.

11. **Concurrent Access**: Must handle entity creation in concurrent scenarios where counter management could be challenging.

12. **Real-world Performance Impact**: Addresses actual performance issues encountered when dealing with many new entities.

13. **Clear Problem Description**: The issue provides excellent explanation of the current problem and proposed solution with clear examples.

14. **Comprehensive Feature Implementation**: This touches entity creation, identity management, string representation, and performance optimization.