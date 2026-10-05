using System.Numerics;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpMultiplayerParty;

/// <summary>
/// A small social party: one player hosts, friends join with an invitation,
/// the leader walks a shared corridor, everyone votes and acts at encounters,
/// and everyone chats throughout. The Engine owns the connections, membership,
/// delivery and chat; this product owns the party rules (<see cref="Party"/>)
/// and what its messages mean. Every player renders the corridor locally from
/// the state the host sends.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string Application = "rusty-engine-party/1";
    private const string UiIntent = "party.ui";
    private const string UiContract = "party.ui.v1";
    private const float Cell = 2f;
    private const float EyeHeight = 0.9f;
    private const float WallHeight = 2.4f;
    private const int UiRefreshSteps = 30;
    private const ulong FloorIdBase = 100, PillarIdBase = 200, WallIdBase = 300, MarkIdBase = 400;

    private readonly IEngineContext engine;
    private readonly ISessionService sessions;
    private readonly UiStream ui;
    private readonly Camera camera;
    private readonly Appearance floorLook, wallLook, pillarLook, voteLook, combatLook;
    private Session? session;
    private Party? party;
    private JsonObject? state;
    private int renderedRevision = -1;
    private string playerName = "";
    private string notice = "";
    private string endedReason = "";
    private bool introduced;
    private ulong uiSequence;
    private ulong steps;
    private bool uiDirty = true;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        sessions = engine.Session;
        floorLook = Primitive(new Color(.25f, .24f, .22f, 1));
        wallLook = Primitive(new Color(.45f, .42f, .38f, 1));
        pillarLook = Primitive(new Color(.35f, .33f, .5f, 1));
        voteLook = Primitive(new Color(.2f, .55f, .9f, 1));
        combatLook = Primitive(new Color(.85f, .25f, .2f, 1));
        camera = engine.CameraView.CreateCamera(CameraAt(1, 1, 0));
        engine.CameraView.SetActiveCamera(camera);
        ui = engine.Ui.OpenStream(new UiStreamRequest("party", "party.panel.v1"));
        PublishUi();
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        HandleIntents(update.Input);
        Observe();
        if (++steps % UiRefreshSteps == 0) uiDirty = true;
        Render();
        if (uiDirty) PublishUi();
        return ProductUpdateResult.None;
    }

    // UI actions made while the Engine is paused act the same way.
    public void HandlePausedIntents(ReadOnlySpan<ProductInputEvent> intents)
    {
        HandleIntents(intents);
        if (uiDirty) PublishUi();
    }

    private void HandleIntents(ReadOnlySpan<ProductInputEvent> intents)
    {
        foreach (ProductInputEvent intent in intents)
        {
            if (intent.Kind != InputEventKind.DirectProductPayload
                || !intent.Intent.Span.SequenceEqual(Encoding.UTF8.GetBytes(UiIntent))) continue;
            JsonNode? action = JsonNode.Parse(intent.PayloadData.Span);
            if (action is not null) Act(action);
        }
    }

    /// <summary>One player action, from the UI or a debug command alike.</summary>
    private void Act(JsonNode action)
    {
        uiDirty = true;
        notice = "";
        string kind = (string?)action["action"] ?? "";
        try
        {
            switch (kind)
            {
                case "host": Host((string?)action["name"], (string?)action["relay"] ?? "", (string?)action["relayToken"] ?? "", (bool?)action["relayOnly"] ?? false); break;
                case "join": Join((string?)action["name"], (string?)action["invitation"] ?? "", (bool?)action["relayOnly"] ?? false); break;
                case "leave": Leave(); break;
                case "chat":
                    if (session is not null) sessions.SendChat(session, (string?)action["text"] ?? "");
                    break;
                default: PartyAction(kind, action); break;
            }
        }
        catch (EngineCallException error)
        {
            notice = error.Diagnostics.IsEmpty ? error.Message : error.Diagnostics.Span[0].Message;
        }
    }

    private void Host(string? name, string relay, string relayToken, bool relayOnly)
    {
        if (!TakeName(name)) return;
        Leave();
        session = sessions.Host(new SessionHostRequest(Identity(), Application, relay, relayToken, relayOnly));
        party = new Party(playerName);
        state = party.Snapshot();
    }

    private void Join(string? name, string invitation, bool relayOnly)
    {
        if (!TakeName(name)) return;
        Leave();
        session = sessions.Join(new SessionJoinRequest(Identity(), Application, invitation, relayOnly));
        introduced = false;
    }

    private void Leave()
    {
        session?.Dispose();
        session = null;
        party = null;
        state = null;
        endedReason = "";
    }

    private bool TakeName(string? name)
    {
        name = (name ?? "").Trim();
        if (name.Length is 0 or > 24 || !name.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_'))
        {
            notice = "Choose a name of letters, digits, - or _ (up to 24).";
            return false;
        }
        playerName = name;
        return true;
    }

    // The player's name doubles as the Engine identity, so a player who
    // rejoins under the same name is recognised as the same member.
    private string Identity() => playerName;

    private void PartyAction(string kind, JsonNode action)
    {
        if (session is null) return;
        // A choice names the moment it was made for, so a click on a button
        // drawn before the party moved on is refused instead of counted.
        JsonObject message = new() { ["type"] = kind };
        foreach (string field in new[] { "character", "move", "choice", "member", "phase" })
            if (action[field] is JsonNode value) message[field] = value.DeepClone();
        if (party is not null)
        {
            notice = party.Apply(Party.HostMember, message) ?? "";
            ShareState();
        }
        else
        {
            sessions.SendToHost(session, Encoding.UTF8.GetBytes(message.ToJsonString()));
        }
    }

    /// <summary>Reads what the session observed since the last call.</summary>
    private void Observe()
    {
        if (session is null) return;
        SessionReadout readout = sessions.Read(session);
        foreach (SessionEvent observed in sessions.TakeEvents(session).Span)
        {
            uiDirty = true;
            switch (observed.Kind)
            {
                case SessionEventKind.Opened when readout.Role == SessionRole.Guest && !introduced:
                    introduced = true;
                    sessions.SendToHost(session, Encoding.UTF8.GetBytes(new JsonObject { ["type"] = "name", ["name"] = playerName }.ToJsonString()));
                    break;
                case SessionEventKind.MemberJoined or SessionEventKind.MemberRejoined when party is not null:
                    party.Connected(observed.Member, true);
                    // A fresh view first: broadcasts reach this member only after it.
                    sessions.SendView(new SessionSendRequest(session, observed.Member, Encode(party.Snapshot())));
                    ShareState();
                    break;
                case SessionEventKind.MemberLeft when party is not null:
                    party.Connected(observed.Member, false);
                    ShareState();
                    break;
                case SessionEventKind.Message when party is not null:
                    JsonNode? action = Parse(observed.Payload);
                    if (action is null) break;
                    string? refusal = party.Apply(observed.Member, action);
                    if (refusal is not null)
                        sessions.Send(new SessionSendRequest(session, observed.Member, Encode(new JsonObject { ["kind"] = "notice", ["text"] = refusal })));
                    ShareState();
                    break;
                case SessionEventKind.View or SessionEventKind.Message:
                    JsonNode? received = Parse(observed.Payload);
                    if ((string?)received?["kind"] == "state") state = received!.AsObject();
                    else if ((string?)received?["kind"] == "notice") notice = (string?)received["text"] ?? "";
                    break;
                case SessionEventKind.Ended:
                    endedReason = $"{observed.EndReason}: {sessions.ReadDiagnosticText(session)}";
                    if (party is null) state = null;
                    break;
            }
        }
    }

    /// <summary>The host sends the whole state whenever it changed.</summary>
    private void ShareState()
    {
        if (party is null || session is null) return;
        if ((int?)state?["revision"] == party.Revision) return;
        state = party.Snapshot();
        sessions.Broadcast(new SessionBroadcastRequest(session, Encode(state)));
        uiDirty = true;
    }

    private static byte[] Encode(JsonObject value) => Encoding.UTF8.GetBytes(value.ToJsonString());

    private static JsonNode? Parse(ReadOnlyMemory<byte> payload)
    {
        try { return JsonNode.Parse(payload.Span); }
        catch (JsonException) { return null; }
    }

    /// <summary>Draws the corridor from the latest state this player holds.</summary>
    private void Render()
    {
        int revision = (int?)state?["revision"] ?? -1;
        if (revision == renderedRevision) return;
        renderedRevision = revision;
        List<AppearanceFact> facts = [];
        if (state is not null)
        {
            ulong id = 0;
            for (int x = 0; x < Party.GridSize; x++)
                for (int z = 0; z < Party.GridSize; z++)
                {
                    bool wall = x == 0 || z == 0 || x == Party.GridSize - 1 || z == Party.GridSize - 1;
                    facts.Add(wall
                        ? Fact(WallIdBase + id, wallLook, new(x * Cell, WallHeight / 2, z * Cell), new(Cell, WallHeight, Cell))
                        : Fact(FloorIdBase + id, floorLook, new(x * Cell, -.05f, z * Cell), new(Cell * .96f, .1f, Cell * .96f)));
                    id++;
                }
            ulong pillar = 0;
            foreach ((int x, int z) in Party.PillarCells)
                facts.Add(Fact(PillarIdBase + pillar++, pillarLook, new(x * Cell, WallHeight / 2, z * Cell), new(Cell * .5f, WallHeight, Cell * .5f)));
            ulong mark = 0;
            foreach ((int x, int z, Phase kind) in Party.EncounterCells)
                facts.Add(Fact(MarkIdBase + mark++, kind == Phase.Combat ? combatLook : voteLook, new(x * Cell, .3f, z * Cell), new(.4f, .4f, .4f)));
            engine.CameraView.UpdateCamera(new(camera, CameraAt((int)state["x"]!, (int)state["z"]!, (int)state["facing"]!)));
        }
        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static CameraDescriptor CameraAt(int x, int z, int facing)
    {
        Vector3 forward = facing switch { 0 => -Vector3.UnitZ, 1 => Vector3.UnitX, 2 => Vector3.UnitZ, _ => -Vector3.UnitX };
        Vector3 right = Vector3.Cross(forward, Vector3.UnitY);
        // Stand at the back of the cell so the cell ahead is in view.
        Vector3 eye = new Vector3(x * Cell, EyeHeight, z * Cell) - forward * (Cell * .45f);
        return new(new(eye, 0, 0), CameraBasisMode.Explicit, new(forward, right, Vector3.UnitY),
            new(CameraProjectionKind.Perspective, 70, 0, .05, 60), CameraViewports.Full);
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), look, true, RenderLayer.Scene);

    private Appearance Primitive(Color color) => engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, color));

    private void PublishUi()
    {
        uiDirty = false;
        engine.Ui.PublishProjection(new UiProjection(ui, ++uiSequence, UiValues.FromJson(Panel())));
    }

    /// <summary>Everything the DOM panel shows; it renders text only.</summary>
    private JsonObject Panel()
    {
        JsonObject panel = new()
        {
            ["name"] = playerName,
            ["notice"] = notice,
            ["characters"] = new JsonArray(Party.Characters.Select(name => (JsonNode)name).ToArray()),
        };
        if (session is null)
        {
            panel["session"] = new JsonObject { ["state"] = "none" };
            return panel;
        }
        SessionReadout readout = sessions.Read(session);
        panel["session"] = new JsonObject
        {
            ["state"] = readout.State.ToString(),
            ["role"] = readout.Role.ToString(),
            ["member"] = readout.LocalMember,
            ["ended"] = endedReason,
            ["invitation"] = readout.Role == SessionRole.Host ? sessions.ReadInvitationText(session) : "",
            ["hostSequence"] = readout.HostSequence,
        };
        panel["members"] = new JsonArray(sessions.ReadMembers(session).ToArray().Select(member => (JsonNode)new JsonObject
        {
            ["member"] = member.Member,
            ["key"] = member.Key.Length > 10 ? member.Key[..10] : member.Key,
            ["host"] = member.IsHost,
            ["local"] = member.IsLocal,
            ["connected"] = member.Connected,
            ["awaitingView"] = member.AwaitingView,
            ["path"] = member.Path.ToString(),
            ["rttMs"] = Math.Round(member.RttMicros / 1000.0, 1),
        }).ToArray());
        panel["chat"] = new JsonArray(sessions.ReadChat(session).ToArray().Select(line => (JsonNode)new JsonObject
        {
            ["member"] = line.Member,
            ["state"] = line.State.ToString(),
            ["text"] = line.Text,
        }).ToArray());
        if (state is not null) panel["party"] = state.DeepClone();
        return panel;
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("party.inspect", Description = "Read-only: this player's session, members, chat and the party state it holds.")]
    public string Inspect() => Panel().ToJsonString(new JsonSerializerOptions { WriteIndented = false });

    [DebugCommand("party.do", Description = "Runs one UI action, the same path the panel's buttons take: a JSON object such as {\"action\":\"move\",\"move\":\"forward\"}.")]
    public string Do(string json)
    {
        JsonNode? action = JsonNode.Parse(json);
        if (action is null) return "not an action";
        Act(action);
        PublishUi();
        return notice == "" ? "ok" : notice;
    }

    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() => Leave();
    public void Shutdown() => Leave();

    public void Dispose()
    {
        Leave();
        engine.Graphics.PublishSnapshot([]);
        ui.Dispose();
        camera.Dispose();
        floorLook.Dispose();
        wallLook.Dispose();
        pillarLook.Dispose();
        voteLook.Dispose();
        combatLook.Dispose();
    }
}
