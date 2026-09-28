using System;
using Rusty.Engine;

namespace SdkPackageConsumer;

/// <summary>
/// Catches operation-local Engine refusals inside one Update and then performs
/// ordinary work in the same callback. The host exercise must keep settling
/// later callbacks; a refusal is only its returned diagnostic.
/// </summary>
internal static class CaughtRefusalChecks
{
    private const string MissingTexture = "missing/caught-refusal.png";
    private const string MissingClip = "missing/caught-refusal.wav";
    private const string PresentTexture = "spatial-particle.png";
    private const ulong NonAdvancingUiSequence = 0;
    private const float OutOfRangeBlendAmount = 2f;

    internal static void Run(IEngineContext engine, UiStream stream)
    {
        ExpectRefusal("Graphics", () => engine.Graphics.OpenResource(new RenderResourceRequest(MissingTexture)));
        ExpectRefusal("Audio", () => engine.Audio.OpenClip(new AudioClipRequest(MissingClip)));
        RenderResourceInfo sky = engine.Graphics.OpenResource(new RenderResourceRequest(PresentTexture));
        ExpectRefusal("CameraView", () => engine.CameraView.SetSkyBackgroundBlend(
            new SkyBackgroundBlendRequest(sky.Handle, sky.Handle, OutOfRangeBlendAmount)));
        StructuredValueNode[] nodes = [new(StructuredValueKind.Null, 0, 0, 0, 0, 0, 0, 0, 0)];
        ExpectRefusal("Ui", () => engine.Ui.PublishProjection(new UiProjection(
            stream,
            NonAdvancingUiSequence,
            new UiValue(nodes, Array.Empty<uint>(), 0, Array.Empty<byte>()))));

        // Valid work after the caught refusals belongs to the same settled callback.
        RenderResourceInfo texture = engine.Graphics.OpenResource(new RenderResourceRequest(PresentTexture));
        if (texture.Handle.Equals(default(RenderResource)))
        {
            throw new InvalidOperationException("resource open after caught refusals returned no handle");
        }

        using Material material = engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(0.8f, 0.2f, 0.2f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false,
            MaterialAlphaMode.Opaque, 0.5f));
        Console.WriteLine("CAUGHT_REFUSAL_CHECKS_PASSED");
    }

    private static void ExpectRefusal(string service, Action call)
    {
        try
        {
            call();
        }
        catch (EngineCallException error)
        {
            if (error.Service != service || error.Diagnostics.Length == 0
                || string.IsNullOrWhiteSpace(error.Diagnostics.Span[0].Code))
            {
                throw new InvalidOperationException(
                    $"{service} refusal lost its named diagnostic: {error.Message}");
            }

            return;
        }

        throw new InvalidOperationException($"{service} accepted a request that must be refused");
    }
}
