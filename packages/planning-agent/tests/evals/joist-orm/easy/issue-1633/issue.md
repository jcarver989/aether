# addReaction hooks not firing after setDefault hook updates a field

# Steps to reproduce

1) Define a `addReaction` hook with a hint (IE: `name` on Author)
2) Define a `setDefault` hook for a field in the hint from step 1 (IE: Author has a default name)
3) call em.flush()

Expected: The `addReaction` hook runs and sees the value from the default
Actual: The `addReaction` hook runs BEFORE the `setDefault` hook, and does not run again after the `setDefault` hook runs. Therefore the `addReaction` hook does not have a chance to process the default `name`.
