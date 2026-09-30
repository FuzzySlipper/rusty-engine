using ReferencedProjects.Transitive;

namespace ReferencedProjects.Library;

public static class LibraryFacts
{
    public const string Revision = "library-1";

    public static string Describe() => $"{Revision};{TransitiveFacts.Revision}";
}
