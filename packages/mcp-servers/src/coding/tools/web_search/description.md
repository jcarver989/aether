Searches the web via Brave Search API for up-to-date information.

## Usage

```json
{"query": "rust async best practices 2024"}
{"query": "tokio documentation", "count": 5, "allowed_domains": ["docs.rs"]}
```

- `query` — **required**, search query (be specific)
- `count` — max results (default: 10, max: 20)
- `allowed_domains` — only include these domains
- `blocked_domains` — exclude these domains

**Returns:** titles, URLs, snippets. Ordered by relevance.

## Tips

- Use snippets to assess relevance before fetching full pages
- Always cite sources using the returned URLs
- Use `web_fetch` to get full content from promising results

## Limitations

- Web pages only (no academic papers, books)
- Results reflect Brave Search's index, not real-time web
- Some content may be behind paywalls
- Rate limited: 2,000 requests/month (free tier)

Requires `BRAVE_SEARCH_API_KEY` environment variable.
