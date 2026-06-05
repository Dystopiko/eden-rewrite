- Enqueue a background job whenever an external service (e.g. Discord) needs to be
  called. Never invoke them directly within the request lifecycle. External services
  are assumed to fail.
- Always use `ApiError` as an error type for all API routes.
- Always follow the convention of serialized code for all `ErrorCode` variants. It
  must be serialized in `SCREAMING_SNAKE_CASE`.
- Be resourceful of finding a suitable error code for specific use.
- Always write error messages that are short, accurate and actionable as it will directly show to
  users using the Eden API and EdenMC mod.
- Always provide vague error messages involving security, authentication and cryptography in
  routes that are accessible in public.
- Keep functions small, composable, and single-purpose by limiting the function bodies of every API
  route function of ONLY 50 lines. Extract the excess into helper functions and always name it
  clearly based on its behavior without reading the implementation.
- Use `CachedRepository` for all high-traffic routes and getter operations. Do not hit the database
  directly when a cached path is available. Only bypass the cache when fresh data is explicitly
  required and justified.
- Log all errors in async tasks and background jobs.
- Always follow `eden_api_types` for all API types. Strictly follow its definitions whenever
  possible. You may write conversions in `src/convert.rs` that converts from schema tables to API 
  structures. If a required type is missing or insufficient, do not improvise. Ask the user for
  confirmation before adding anything to the crate.
- Always structure new routes as directories under `src/controllers` directory, mirroring the route
  path as nested components (e.g. `/admin/@me/accounts -> ./admin/me/accounts.rs`).
- Always name route handler functions after their HTTP method (e.g. get, post, patch, delete).
- Tests for each route must be written ONLY in `eden-integration-tests` crate.
