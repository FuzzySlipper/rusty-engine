namespace Rusty.Engine;

/// <summary>Where the product UI loads an image granted with <c>Ui.OpenImage</c>.</summary>
public static class UiImageUrls
{
    private const string Route = "/__rusty/product/runtime/ui-images/";

    /// <summary>
    /// The same-origin URL the product UI uses as an <c>&lt;img&gt;</c> source, for example
    /// carried in a projection. The host serves the image there until the image is disposed,
    /// and answers 404 afterwards.
    /// </summary>
    public static string Url(this UiImage image)
    {
        ArgumentNullException.ThrowIfNull(image);
        return Route + image.Handle.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
    }
}
