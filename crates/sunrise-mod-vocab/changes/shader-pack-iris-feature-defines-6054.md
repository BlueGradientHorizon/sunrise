# Pack defines may not name IRIS_FEATURE_*

`PackBinding::validate_syntax` now refuses a define named `IRIS_FEATURE_*`, as it refuses `GL_` names. Those macros
are the engine's answers to the feature flags a shader pack lists in `iris.features.optional` and
`iris.features.required`: the engine defines one for each flag it supports that the pack lists, and drops any other
before reading the pack, so a mod's define of such a name never reached the pack. A mod that declared one now gets a
registration error naming it; drop the define, or use a name of the mod's own.
