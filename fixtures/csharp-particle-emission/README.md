# Packaged particle emission regression

`ParticleEmissionChecks.cs` runs in the immutable SDK consumer's first admitted
Update via `scripts/test-csharp-sdk-package.sh --coreclr-smoke`.

It catches named refusals for the task #8714 minimal descriptor, invalid enums,
missing curves/frame count and an out-of-range seed. It then admits cube,
billboard and AABB-colliding debris bursts, and a repeated signal label. The host exercise must
observe the same update's UI publication and complete subsequent callbacks.
This checks generated marshalling, native admission and callback settlement;
it does not check rendered appearance or diagnose an unrelated shutdown signal.
