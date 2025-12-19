# Planning Agent

An LLM-powered coding agent evaluation framework for assessing plan quality. Currently evaluates how well LLMs can analyze GitHub/Linear issues and generate implementation plans. The long-term vision is to use these plans to guide autonomous execution agents in creating pull requests.

## Hypothesis

Better upfront planning and research leads to higher quality code output. Rather than having an execution agent directly generate code from task descriptions, we hypothesize that a dedicated planning phase will result in:

- More comprehensive understanding of requirements
- Better identification of relevant code locations
- Clearer implementation strategies
- Fewer bugs and rework cycles

## Architecture

The planning agent is built on top of the Aether framework and uses several key components:

- **Crucible**: Evaluation framework for running and scoring agent performance
- **MCP (Model Context Protocol)**: Provides dynamic tool discovery for file operations, search, and codebase analysis
- **Multi-LLM Support**: Works with OpenRouter (Claude, GPT-4, etc.) and Ollama (local models)
- **Git-based Evaluation**: Uses real PRs from high-quality repositories as gold standards

### How It Works: Aspirational (End Goal)

The full vision includes both planning and execution phases:

```
Issue Description → Planning Agent → plan.md → Execution Agent → Code Output
                                                                       ↓
                                                         Compare with Real PR Diff
```

1. **Input**: GitHub issue description from a closed, merged PR
2. **Planning Phase**: Agent analyzes codebase and creates detailed implementation plan
3. **Execution Phase**: Separate agent implements code based on the plan
4. **Evaluation**: Compare generated code diff against the actual merged PR (gold standard)

This would provide **end-to-end validation** that good plans actually produce good code.

### How It Works: Current Reality

Currently, only the planning phase is implemented:

```
Issue Description → Planning Agent → plan.md
                         ↓
                   (cloned repo at start_commit)
                         ↓
              LLM Judge evaluates plan against actual PR diff
```

1. **Input**: GitHub issue description from a closed, merged PR
2. **Setup**: Clone repository and checkout to `start_commit` (pre-PR state)
3. **Planning Phase**: Agent analyzes codebase and writes `plan.md` with implementation strategy
4. **Evaluation**: LLM judge scores the plan (1-10) based on:
   - How well it identifies the right files and functions to modify
   - Whether the proposed changes match the actual PR diff
   - Likelihood that following the plan would produce the gold standard code

**What's Missing**: No execution agent exists yet. Plans are judged on quality, but never actually tested by implementing them and comparing the output.

## Evaluation Methodology

We evaluate using real GitHub issues from the [Joist ORM](https://github.com/joist-orm/joist-orm) repository:

- **Gold Standard**: Actual merged PRs authored by senior engineers (primarily the Joist maintainer)
- **Pre-PR State**: Git commit SHA before the PR was merged (`start_commit`)
- **Post-PR State**: Git commit SHA after the PR was merged (`eval_commit`)
- **Current Evaluation**: LLM judge compares plan quality against the actual PR diff
- **Future Evaluation**: (Not yet implemented) Compare execution agent's generated diff vs. actual PR diff

### Eval Cases

Cases are categorized by complexity:

**Easy (4 cases)**
- Configuration changes
- Simple renames and refactoring
- Straightforward feature additions

**Medium (3 cases)**
- ORM logic improvements
- Performance optimizations
- Type system enhancements

**Hard (5 cases)**
- Complex architectural changes
- Advanced type system work
- Major feature implementations

See [eval-cases/README.md](eval-cases/README.md) for detailed case descriptions.

## Getting Started

### Prerequisites

- Rust toolchain (2024 edition)
- An LLM provider API key (OpenRouter, Z.ai, Ollama, Llama.cpp, OpenAI and Anthropic are supported)

### Installation

From the workspace root:

```bash
cargo build -p planning-agent
```

### Running Evaluations

Basic usage with default model:

```bash
cargo run -p planning-agent
```

With a specific model:

```bash
# Using OpenRouter
cargo run -p planning-agent -- --model openrouter:anthropic/claude-sonnet-4-5-20250929

# Using Ollama (local)
cargo run -p planning-agent -- --model ollama:llama3.3
```

Advanced options:

```bash
# Custom batch size and delay
cargo run -p planning-agent -- \
  --model openrouter:anthropic/claude-sonnet-4-5-20250929 \
  --batch-size 5 \
  --batch-delay 3

# Use different models for agent and judge
cargo run -p planning-agent -- \
  --model openrouter:anthropic/claude-sonnet-4-5-20250929 \
  --judge-model openrouter:openai/gpt-4-turbo

# Custom directories
cargo run -p planning-agent -- \
  --evals-dir ./my-evals \
  --output-dir ./my-results
```

### Viewing Results

By default, a web server starts on `http://localhost:3000` showing an interactive report with:

- Pass/fail rates based on LLM judge scores
- Detailed traces for each evaluation (agent's planning process)
- The actual git diff (gold standard) for comparison
- Judge's reasoning and feedback on plan quality
- Agent's generated `plan.md` files

To disable the web server:

```bash
cargo run -p planning-agent -- --no-serve
```

### Current Limitations

Since only the planning phase is implemented:

- **No proof plans work**: Plans are scored subjectively by an LLM judge, not validated by actual code generation
- **Judge variability**: Different judge models may score the same plan differently
- **No iterative refinement**: If a plan scores poorly, there's no feedback loop to improve it
- **Hypothesis untested**: We can't yet prove that better plans → better code, only that plans *look* reasonable

To fully validate the hypothesis, we need to implement the execution agent and measure actual code quality.

## CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--model` | `-m` | `zai:GLM-4.6` | Model spec for the planning agent |
| `--judge-model` | `-j` | Same as `--model` | Model spec for evaluating results |
| `--batch-size` | `-b` | `3` | Number of evals to run concurrently |
| `--batch-delay` | `-d` | `2` | Delay in seconds between batches |
| `--evals-dir` | `-e` | `./tests` | Directory containing evaluations |
| `--output-dir` | `-o` | `./eval-results` | Directory for results output |
| `--no-serve` | | `false` | Disable web server for results |

## Project Structure

```
planning-agent/
├── src/
│   └── main.rs                    # Entry point and CLI
├── tests/
│   ├── AGENTS.md                  # Planning agent instructions
│   ├── mcp.json                   # MCP server configuration
│   └── evals/                     # Evaluation test cases
│       └── joist/
│           ├── easy/
│           ├── medium/
│           └── hard/
├── eval-cases/                    # Issue metadata and rationale
│   ├── README.md
│   ├── easy/
│   ├── medium/
│   └── hard/
└── eval-results/                  # Generated evaluation outputs
    ├── summary.json
    ├── traces.jsonl
    ├── results/
    └── report/
```

## Adding New Evaluations

1. Find a closed GitHub issue with a merged PR from a high-quality codebase
2. Create a case file in `eval-cases/{difficulty}/{issue-number}.md` documenting why it's a good eval
3. Create the eval structure in `tests/evals/{repo}/{difficulty}/issue-{number}/`
4. Add `eval.json` with git configuration:
   ```json
   {
     "git": {
       "url": "https://github.com/org/repo",
       "start_commit": "abc123...",  // Commit before PR was merged
       "eval_commit": "def456..."     // Commit after PR was merged
     },
     "assertions": [
       {
         "type": "LLMJudge",
         "data": {
           "prompt": "Evaluate if the plan would produce the actual code..."
         }
       }
     ]
   }
   ```
5. Add `prompt.md` containing the GitHub issue description (what the agent sees)
6. Run evaluations and assess plan quality via LLM judge



## Related Packages

- **crucible**: Evaluation framework powering the test runner
- **aether**: Core LLM and MCP integration
- **mcp-lexicon**: MCP tool definitions for coding tasks
