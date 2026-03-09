Rates an agent-authored skill as helpful or harmful.

## Usage

```json
{"name": "skill-name", "helpful": true}
```

- `name` — **required**, skill directory name
- `helpful` — **required**, `true` if helpful, `false` if harmful

## When to Rate

- **Helpful** — skill helped you accomplish your task correctly
- **Harmful** — skill was wrong, misleading, or caused a mistake

Skills with too many harmful ratings are automatically pruned. Only agent-authored skills can be rated.
