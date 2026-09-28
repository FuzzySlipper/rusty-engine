# Packaged caught-refusal regression

`CaughtRefusalChecks.cs` runs in the immutable SDK consumer's first admitted
Update via `scripts/test-csharp-sdk-package.sh --coreclr-smoke`.

It catches named Graphics, Audio, CameraView, Dynamics and UI refusals, then
opens a resource and creates a material in the same callback. The host exercise
must observe that update's UI publication and keep settling later callbacks: a
caught refusal is only its returned diagnostic and does not taint the product.
