using System;

namespace Rusty.Engine.Resolution;

/// <summary>
/// Product-owned mutation boundary used when an operation supports Preview and Apply. It is a
/// direct local contract, not an Engine-wide command bus or generic resolution protocol.
/// </summary>
public interface IResolutionTransaction
{
    void Stage();
    void Commit();
    void Abort();
}
