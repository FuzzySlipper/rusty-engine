using System.Text;

namespace Rusty.Engine;

/// <summary>
/// Text readouts and common sends for a multiplayer <see cref="Session"/>.
/// </summary>
public static class SessionConvenience
{
    /// <summary>The host's member number in every session.</summary>
    public const uint HostMember = 1;

    /// <summary>The invitation guests join with; empty until the host is open.</summary>
    public static string ReadInvitationText(this ISessionService sessions, Session session)
    {
        ArgumentNullException.ThrowIfNull(sessions);
        return Encoding.UTF8.GetString(sessions.ReadInvitation(session).Span);
    }

    /// <summary>Why the session ended; empty while it runs.</summary>
    public static string ReadDiagnosticText(this ISessionService sessions, Session session)
    {
        ArgumentNullException.ThrowIfNull(sessions);
        return Encoding.UTF8.GetString(sessions.ReadDiagnostic(session).Span);
    }

    /// <summary>A guest's message to the host.</summary>
    public static SessionSendReceipt SendToHost(this ISessionService sessions, Session session, ReadOnlyMemory<byte> payload)
    {
        ArgumentNullException.ThrowIfNull(sessions);
        return sessions.Send(new SessionSendRequest(session, HostMember, payload));
    }

    /// <summary>Adds a chat line; see <see cref="ISessionService.ReadChat"/> for its state.</summary>
    public static ulong SendChat(this ISessionService sessions, Session session, string text)
    {
        ArgumentNullException.ThrowIfNull(sessions);
        return sessions.SendChat(new SessionChatRequest(session, text)).LocalId;
    }
}

/// <summary>Text readouts for <see cref="IHttpService"/>.</summary>
public static class HttpConvenience
{
    /// <summary>Why a transfer failed; empty otherwise.</summary>
    public static string ReadDiagnosticText(this IHttpService http, HttpTransfer transfer)
    {
        ArgumentNullException.ThrowIfNull(http);
        return Encoding.UTF8.GetString(http.ReadDiagnostic(transfer).Span);
    }

    /// <summary>A completed GET's body as UTF-8 text.</summary>
    public static string ReadBodyText(this IHttpService http, HttpTransfer transfer)
    {
        ArgumentNullException.ThrowIfNull(http);
        return Encoding.UTF8.GetString(http.ReadBody(transfer).Span);
    }

    /// <summary>The final response's header, or null; names are lower case.</summary>
    public static string? ReadHeader(this IHttpService http, HttpTransfer transfer, string name)
    {
        ArgumentNullException.ThrowIfNull(http);
        foreach (HttpHeader header in http.ReadHeaders(transfer).Span)
            if (string.Equals(header.Name, name, StringComparison.OrdinalIgnoreCase)) return header.Value;
        return null;
    }
}
