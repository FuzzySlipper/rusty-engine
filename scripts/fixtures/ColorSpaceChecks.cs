using System;
using Rusty.Engine;

// The SDK's sRGB helpers (#9845) decode as the renderer does (render-wgpu
// `srgb_to_linear`, pinned at the same values by its
// `srgb_decodes_as_the_sdk_helper_does` test), and encode back.
internal static class ColorSpaceChecks
{
    private const float Tolerance = 1e-6f;

    internal static void Run()
    {
        foreach ((float srgb, float linear) in new[] { (0f, 0f), (0.04045f, 0.0031308050f), (0.5f, 0.21404114f), (1f, 1f) })
        {
            Require(Near(Color.SrgbToLinear(srgb), linear), $"sRGB {srgb} decodes to {Color.SrgbToLinear(srgb)}, not {linear}");
            Require(Near(Color.LinearToSrgb(linear), srgb), $"linear {linear} encodes to {Color.LinearToSrgb(linear)}, not {srgb}");
        }

        Color decoded = Color.FromSrgb(0.5f, 1, 0, 0.25f);
        Require(Near(decoded.R, 0.21404114f) && decoded.G == 1 && decoded.B == 0 && decoded.A == 0.25f, $"FromSrgb gave {decoded}");
        Color hex = Color.FromSrgbHex(0x80FF00);
        Require(Near(hex.R, Color.SrgbToLinear(128 / 255f)) && hex.G == 1 && hex.B == 0 && hex.A == 1, $"FromSrgbHex gave {hex}");
        Color back = decoded.ToSrgb();
        Require(Near(back.R, 0.5f) && Near(back.G, 1) && back.A == 0.25f, $"ToSrgb gave {back}");
        bool refused = false;
        try { Color.FromSrgbHex(0x1000000); }
        catch (ArgumentOutOfRangeException) { refused = true; }
        Require(refused, "FromSrgbHex took a code past 0xFFFFFF");
    }

    private static bool Near(float actual, float expected) => MathF.Abs(actual - expected) <= Tolerance;

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
