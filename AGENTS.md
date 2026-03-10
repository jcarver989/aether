# Code Style

1. Backwards compatibility and fallbacks are not a concern when planning or implementing code.
2. Favor simplicity over complexity. Good architecture results in simple systems.
3. Non-doc (double slash) comments should be used extremely sparringly and only to explain the "why?" a certain piece of code exits. Comments should never be used for file separators.

# Testing

1. When fixing a bug, always write test(s) first. Confirm the test(s) fail _before_ attempting to fix the bug.
2. Prefer integration tests that assert against state using fakes (e.g. in memory file system that asserts file contents) over mocks that test behavior (e.g. how many times did we call write_file?)
3. Don't put timeouts into tests, this always leads to flaky tests on CPUs with different speeds.
