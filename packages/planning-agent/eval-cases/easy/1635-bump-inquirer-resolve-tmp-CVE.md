# PR 1635: Bump inquirer to resolve `tmp` CVE

## Issue Information
- **Issue**: #1634 - "joist-codegen/joist-graphql-codegen transitively require a version of `tmp` with a CVE"
- **Issue Link**: https://github.com/joist-orm/joist-orm/issues/1634
- **Issue Title**: joist-codegen/joist-graphql-codegen transitively require a version of `tmp` with a CVE

## PR Information  
- **PR**: #1635
- **PR Link**: https://github.com/joist-orm/joist-orm/pull/1635
- **PR Title**: fix: bump inquirer to resolve `tmp` CVE
- **Author**: Ben Limmer (blimmer)

## Commit Information
- **Commit SHA Prior to PR Merge**: `7a8b9c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a`
- **Commit SHA After PR Merge**: `07e2b3464e2528848b2c5ba1a1575da2f26aae70`

## Complexity Rating
easy

## Why This Is a Good Eval Candidate

This is an excellent easy-level eval case because:

1. **Clear Security Issue**: The issue reports a specific CVE (GHSA-52f5-9888-hmc6) in a transitive dependency, making the problem unambiguous.

2. **Dependency Analysis Required**: The solution requires analyzing the dependency tree to identify the vulnerable path (`inquirer@9.3.7 → external-editor@3.1.0 → tmp@0.0.33`).

3. **Modern Migration**: Instead of a simple version bump, this migrates to the modern `@inquirer/prompts` package, demonstrating best practices.

4. **Multiple Benefits**: The fix addresses security while also providing cleaner API, native TypeScript types, and removing `@types/inquirer` dependency.

5. **Well-Documented Solution**: The PR provides clear problem statement, solution approach, and explains the benefits of the migration.

6. **Easy Verification**: Success can be verified through security scans, dependency tree analysis, and functionality testing.

7. **Real-world Security Importance**: This represents a common scenario where transitive dependencies introduce vulnerabilities requiring careful management.

8. **Modern Package Management**: Demonstrates understanding of modern JavaScript ecosystem and package evolution patterns.

9. **Backward Compatibility**: Must ensure existing functionality continues to work with the new inquirer package.

10. **Clear Impact Assessment**: The issue shows exact dependency chains and vulnerability, making the scope clear and measurable.