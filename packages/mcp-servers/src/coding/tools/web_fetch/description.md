Fetches content from a URL, converting HTML to Markdown.

## Usage

```json
{"url": "https://docs.rs/serde"}
{"url": "https://example.com", "prompt": "Find API documentation", "timeout": 60000}
```

- `url` — **required**, HTTP/HTTPS URL (HTTP auto-upgraded)
- `prompt` — describe what you're looking for (optional)
- `timeout` — max wait in ms (default: 30000, max: 60000)

**Returns:** Markdown content, final URL (after redirects), status code, page title, truncated flag.

## Tips

- Use specific URLs rather than relying on search
- Handle 4xx/5xx status codes gracefully

## Limitations

- No JavaScript execution (can't handle SPAs)
- No cookie/session handling (can't access authenticated pages)
