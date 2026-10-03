namespace Rusty.Engine;

/// <summary>Where the product UI loads a font granted with <c>Ui.OpenFont</c>.</summary>
public static class UiFontUrls
{
    private const string Route = "/__rusty/product/runtime/ui-fonts/";

    /// <summary>
    /// The same-origin URL the product UI uses in a CSS <c>@font-face</c> <c>src</c>, for
    /// example carried in a projection. The host serves the font there with its own
    /// content type until the font is disposed, and answers 404 afterwards.
    /// </summary>
    public static string Url(this UiFont font)
    {
        ArgumentNullException.ThrowIfNull(font);
        return Route + font.Handle.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
    }
}
