# Joist ORM Eval Cases

This directory contains high-quality GitHub issues with merged PRs for evaluating LLM-powered coding agents. The cases are categorized by complexity and represent real-world maintenance and feature development tasks from the Joist ORM codebase.

## Structure

- `easy/` - Simple configuration changes, renames, or straightforward fixes
- `medium/` - ORM logic improvements, performance optimizations, type system enhancements  
- `hard/` - Complex architectural changes, advanced type system work, major feature implementations

## Selection Criteria

All selected cases meet these criteria:

1. **High-Quality Code**: PRs authored by senior engineers (primarily Stephen Haberman, the Joist maintainer)
2. **Existing GitHub Issues**: Every case has a corresponding issue with clear requirements
3. **Comprehensive Testing**: PRs include good automated test coverage
4. **Real-World Relevance**: Addresses practical needs encountered in ORM development
5. **Clear Success Criteria**: Well-defined requirements and measurable outcomes
6. **Senior Engineering**: Demonstrates best practices and architectural thinking

## Case Summary

### Easy Cases (4)
1. **1610** - Mark docs package private (configuration change)
2. **1482** - Rename createOrUpdatePartial to upsert (API refactoring)
3. **1406** - Add includeSchema option to config (feature addition)
4. **1635** - Bump inquirer to resolve tmp CVE (security fix)

### Medium Cases (3)
1. **1520** - Allow createdAt for m2m tables (ORM logic fix)
2. **1419** - Add isLoaded caching (performance optimization)
3. **1465** - Fix subtype pruning for STI m2os (query optimization)

### Hard Cases (5)
1. **1539** - Add raw conditions to aliases (advanced query building)
2. **1620** - Support ESM in graphql-codegen (module system integration)
3. **1550** - Add em.fork (entity manager duplication)
4. **1444** - Fix subtype specialization type errors (advanced TypeScript)
5. **1652** - Make em.create ids stable in toString (entity lifecycle)

## Usage

Each case file contains:
- Issue information (title, link, description)
- PR information (title, link, author)
- Commit SHAs (before/after merge)
- Complexity rating
- Rationale for why it's a good eval candidate

These cases provide a diverse range of challenges for testing planning agents, from simple configuration changes to complex architectural decisions.