namespace Rusty.Engine.Persistence;

/// <summary>Why the Engine could not read or write a stored value.</summary>
public enum PersistenceStorageFailure
{
    /// <summary>The file is not a current container, for example one written in a retired layout. The Engine never migrates it: discard or convert it.</summary>
    UnrecognizedContainer,
    /// <summary>The file is a current container whose header and payload disagree.</summary>
    MalformedContainer,
    /// <summary>The file system refused the read or write.</summary>
    Io,
}

/// <summary>
/// An expected storage refusal from <see cref="ProductStateStore{TState}"/>. The stored bytes,
/// revision and store are unchanged, so a product can report it and carry on. The inner
/// <see cref="EngineCallException"/> carries the Engine diagnostic.
/// </summary>
public sealed class PersistenceStorageException : Exception
{
    private PersistenceStorageException(PersistenceStorageFailure failure, EngineCallException error)
        : base(error.Message, error)
    {
        Failure = failure;
    }

    public PersistenceStorageFailure Failure { get; }

    internal static T Refuse<T>(Func<T> operation)
    {
        try
        {
            return operation();
        }
        catch (EngineCallException error) when (Classify(error) is { } failure)
        {
            throw new PersistenceStorageException(failure, error);
        }
    }

    private static PersistenceStorageFailure? Classify(EngineCallException error)
    {
        if (error.Service != "Persistence" || error.Diagnostics.Length != 1) return null;
        return error.Diagnostics.Span[0].Code switch
        {
            "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED" => PersistenceStorageFailure.UnrecognizedContainer,
            "CSHARP_PERSISTENCE_CONTAINER_MALFORMED" => PersistenceStorageFailure.MalformedContainer,
            "CSHARP_PERSISTENCE_IO" => PersistenceStorageFailure.Io,
            _ => null,
        };
    }
}
