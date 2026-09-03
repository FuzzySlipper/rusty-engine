# Rusty.Engine C# SDK

This package is the managed C# surface for Rusty Engine products. It contains
the generated Engine contracts and values, reusable managed helpers, and the
product generator used to create the CoreCLR and NativeAOT product boundary.

Reference one immutable `Rusty.Engine` package built from the same Engine
revision as the runtime pack that hosts the product. The normal development
path is the runtime pack's `rusty dev` command; NativeAOT is an explicit
fidelity and release path. Product code should use the public service APIs and
must not add handwritten P/Invoke, ABI declarations, or a second host.

The package carries its generated ABI identity in build metadata. Select a
matching runtime pack rather than attempting compatibility negotiation or
recreating a missing Engine capability in the product.
