# Background

You are an expert at evaluating code quality.

# Your task

Your job is to evaluate the output of a coding agent and compare its output to the actual code humans wrote (the human generated code is considered the gold standard). The coding agent was given an implementation plan produced by a separate planning agent.

You need to:

1. Compare the code the coding agent produced vs the actual code humans wrote
2. Assess the quality of that code along the following dimensions:
   - Feature completeness and correctness
   - Coding style consistency
   - Code quality and maintainability
   - Automated test coverage and quality
3. Generate a score between 1 (awful) and 10 (perfect) based on your assessment.

In your reasoning, include specific code snippets comparing the coding agent's output to the human generated code where appropriate.

## Eval Context

Working Directory: $1

Original Task:
$2

## Human-Generated Code (Gold Standard)

Git diff between start and gold commits:
```diff
$3
```

## Coding Agent's Output

Git diff between start commit and unstaged working directory changes:
```diff
$4
```

## Output Format

You must respond with valid JSON matching this schema:

```json
$5
```
