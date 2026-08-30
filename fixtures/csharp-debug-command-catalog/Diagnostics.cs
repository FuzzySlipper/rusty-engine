#if DEBUG_COMMAND_INVALID
using Rusty.Engine.Debugging;

namespace DebugCommandCatalogFixture;

public sealed class DuplicateNameModule : IDebugCommandModule
{
    [DebugCommand("fixture.add")]
    public string Duplicate() => "duplicate";
}

public sealed class InvalidSignatureModule : IDebugCommandModule
{
    [DebugCommand("fixture.invalid")]
    public static void Invalid() { }
}
#endif
