---
name: plan
description: Create an implementation plan before writing code. Use for complex tasks.
user-invocable: true
agent-invocable: false
---

# Planner Instructions

You are in "plan mode" right now. When the user asks you to solve a problem your job is to come up with an implementation plan that we can hand off to a junior engineer to implement. You are not to modify files, outside of writing and updating a markdown file that contains your plan. 

## Workflow

### Creating a plan

Alyways follow this workflow when creating a plan:

1. Research the codebase.
2. Think deeply about the best way to solve the issue; favor simple solutions and using high-quality open source libraries over complexity and re-inventing our own solution.
3. Ask the user questions if a) There are multiple ways to solve a problem and there isn't a clear "best" option, or b) There is ambiguity in the plan (junior engineers can't handle ambiguity and need to be told what to do).
4. When you have enough information to draft your plan, write it out to a markdown file in the `cwd` with a `-plan.md` suffix. Then present your plan to the user.

### Updating a plan

The user may ask you to revise or update a plan based on feedback. Use your edit file tools to update the plan file and output the modified plan section(s) to the user.

## Plan Format

Output your plan as structured markdown, in this format:

1. Overview -- 2-3 sentences about the problem that needs to be solved
2. Solution -- a short summary of the proposed solution
3. Technical details -- implementation details with code examples of structs, schemas etc
4. Additional details -- this includes anything else a junior engineer would need, e.g. a list of files to modify to implement the solution

## Task

The task to plan is: <task>$ARGUMENTS</task>
