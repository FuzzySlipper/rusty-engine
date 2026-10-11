using System;

namespace Rusty.Engine;

public readonly partial record struct Color
{
    /// <summary>
    /// A colour given in sRGB (a palette, a colour picker, a CSS or hex code) as the linear colour
    /// the Engine takes: each channel decoded with the sRGB transfer function, as the renderer
    /// decodes an sRGB texture. Alpha is coverage and stays as given.
    /// </summary>
    public static Color FromSrgb(float r, float g, float b, float a = 1) =>
        new(SrgbToLinear(r), SrgbToLinear(g), SrgbToLinear(b), a);

    /// <summary>
    /// <see cref="FromSrgb(float, float, float, float)"/> from a <c>0xRRGGBB</c> code, as CSS and
    /// image editors write it.
    /// </summary>
    public static Color FromSrgbHex(int rgb, float a = 1)
    {
        if ((rgb & ~0xFFFFFF) != 0) throw new ArgumentOutOfRangeException(nameof(rgb), rgb, "an sRGB code is 0xRRGGBB");
        return FromSrgb(((rgb >> 16) & 0xFF) / 255f, ((rgb >> 8) & 0xFF) / 255f, (rgb & 0xFF) / 255f, a);
    }

    /// <summary>This linear colour encoded as sRGB (for a label or a CSS colour); alpha as it is.</summary>
    public Color ToSrgb() => new(LinearToSrgb(R), LinearToSrgb(G), LinearToSrgb(B), A);

    /// <summary>One sRGB channel (0 to 1) as linear light.</summary>
    public static float SrgbToLinear(float value) =>
        value <= 0.04045f ? value / 12.92f : MathF.Pow((value + 0.055f) / 1.055f, 2.4f);

    /// <summary>One linear channel (0 to 1) encoded as sRGB.</summary>
    public static float LinearToSrgb(float value) =>
        value <= 0.0031308f ? value * 12.92f : (1.055f * MathF.Pow(value, 1 / 2.4f)) - 0.055f;
}
