# Web Search Tool

Searches the web for up-to-date information using the Brave Search API. Returns structured search results with titles, URLs, and snippets that can be used to ground responses with current information.

**Use cases:**
- Finding recent news or developments
- Getting up-to-date documentation
- Researching current events or facts
- Finding tutorials or guides
- Grounding responses with multiple sources

**When to use:**
- When you need information that may have changed since your training cutoff
- When you need to verify current facts
- When looking for recent developments or news
- When you need specific, up-to-date technical documentation

**When NOT to use:**
- For simple factual queries that don't require verification
- When you already have the information you need
- For creative tasks that don't require factual accuracy

**Parameters:**
- `query` (required): The search query - be specific and use relevant keywords
- `count` (optional): Number of results to return, default: 10, max: 20
- `allowed_domains` (optional): Only return results from these specific domains
- `blocked_domains` (optional): Exclude results from these domains

**Important notes:**
- Always cite your sources using the URLs provided in results
- The API is rate-limited (2,000 free requests/month on Brave)
- Use the snippets to understand relevance before fetching full pages
- Domain filtering is applied after the search, so blocking domains may reduce result count
- Results are ordered by relevance according to the search engine

**Limitations:**
- Search results are limited to web pages (no academic papers, books, etc.)
- Results reflect what's indexed by Brave Search
- Some content may be behind paywalls or require authentication
- Search is performed against a web index, not in real-time

**API Configuration:**
Requires the `BRAVE_SEARCH_API_KEY` environment variable. Get a free API key from <https://api.search.brave.com/app/keys> (free tier: 2,000 requests/month).
