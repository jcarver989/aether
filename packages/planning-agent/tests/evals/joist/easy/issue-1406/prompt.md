codegen: optionally define schemas to derive metadata from

Long time, no issue. :)

We're evaluating using pg schemas for some lightweight tenancy. This isn't external/per organisation, but really for creating various ops - such as a "demo" tenant.

The idea is to have a couple schemas say public, demo etc. We have some global tables, which we are considering moving out to a `core` schema.

We'll then be able to set the pg search_path to `{tenant}, core` per request.

Within codegen there's currently a function that checks if a schema name equals "public". The request is to make this optionally configurable within joist-config.json to an array of schemas, enabling definitions like `['public', 'core']`.

nothing else from joist would be needed here, as search_paths allows this to be handled at pg level.

Happy to PR if you're open to this!
