using System.Text.Json.Nodes;

namespace CsharpMultiplayerParty;

/// <summary>
/// The fixture's party rules. Only the network host runs them; guests see the
/// state the host sends. The party leader is a rule of this product, not of
/// the session: leadership can pass to a guest while the host stays the host.
/// </summary>
internal sealed class Party
{
    public const uint HostMember = 1;
    public const int GridSize = 7;
    public static readonly string[] Characters = ["Fighter", "Cleric", "Mage", "Thief"];
    private const int LogLines = 8;
    private static readonly (int X, int Z)[] Pillars = [(2, 2), (4, 2), (2, 4), (4, 4)];
    private static readonly Encounter[] Encounters =
    [
        new(3, 1, Phase.Vote, "A stranger asks the party for help.", ["Help", "Ignore", "Rob"]),
        new(5, 4, Phase.Combat, "Goblins leap from the shadows!", ["Attack", "Defend", "Flee"]),
        new(1, 5, Phase.Vote, "The corridor forks.", ["Left", "Right"]),
    ];

    private readonly Dictionary<uint, Seat> seats = new();
    private readonly HashSet<(int, int)> resolvedTiles = new();
    private readonly Dictionary<uint, int> choices = new();
    private readonly List<string> log = new();
    private Encounter? encounter;

    public int Revision { get; private set; }
    public int PhaseId { get; private set; }
    public Phase Current { get; private set; } = Phase.Explore;
    public int X { get; private set; } = 1;
    public int Z { get; private set; } = 1;
    /// <summary>0 north (−Z), 1 east, 2 south, 3 west; the party starts facing down the corridor.</summary>
    public int Facing { get; private set; } = 1;
    public uint Leader { get; private set; } = HostMember;

    public Party(string hostName)
    {
        seats[HostMember] = new Seat(hostName) { Connected = true };
        Note($"{hostName} hosts the party.");
    }

    public static bool Blocked(int x, int z) =>
        x <= 0 || z <= 0 || x >= GridSize - 1 || z >= GridSize - 1 || Pillars.Contains((x, z));

    public static IEnumerable<(int X, int Z)> PillarCells => Pillars;
    public static IEnumerable<(int X, int Z, Phase Kind)> EncounterCells => Encounters.Select(e => (e.X, e.Z, e.Kind));

    public void Connected(uint member, bool connected)
    {
        if (!seats.TryGetValue(member, out Seat? seat))
        {
            // Named by its first message; announced then.
            seats[member] = new Seat($"Player {member}") { Connected = connected };
            Changed();
            return;
        }
        if (seat.Connected == connected) return;
        seat.Connected = connected;
        Note(connected ? $"{seat.Name} is back." : $"{seat.Name} is away.");
        // An absent leader cannot lead: the host leads until it is passed on.
        if (!connected && Leader == member)
        {
            Leader = HostMember;
            Note($"{seats[HostMember].Name} leads while {seat.Name} is away.");
        }
    }

    /// <summary>Applies one member's action; returns why it was refused, if it was.</summary>
    public string? Apply(uint member, JsonNode action)
    {
        if (!seats.TryGetValue(member, out Seat? seat)) return "unknown member";
        string type = (string?)action["type"] ?? "";
        int? phase = (int?)action["phase"];
        if (phase is not null && phase != PhaseId && type is "vote" or "act" or "resolve")
            return $"that choice was for an earlier moment (phase {phase}, now {PhaseId})";
        switch (type)
        {
            case "name":
                string name = ((string?)action["name"] ?? "").Trim();
                if (name.Length is 0 or > 24) return "a name has 1 to 24 characters";
                bool first = seat.Name == $"Player {member}";
                seat.Name = name;
                if (first) Note($"{name} is here.");
                else Changed();
                return null;
            case "claim":
                string character = (string?)action["character"] ?? "";
                if (!Characters.Contains(character)) return $"no character {character}";
                if (seats.Values.Any(other => other != seat && other.Character == character))
                    return $"{character} is taken";
                seat.Character = character;
                Note($"{seat.Name} plays the {character}.");
                return null;
            case "lead":
                if (member != Leader) return "only the leader passes leadership";
                uint next = (uint?)action["member"] ?? 0;
                if (!seats.TryGetValue(next, out Seat? heir) || !heir.Connected) return "that member is not here";
                Leader = next;
                Note($"{heir.Name} now leads.");
                return null;
            case "move":
                if (member != Leader) return "only the leader moves the party";
                if (Current != Phase.Explore) return "the party is busy";
                return Move((string?)action["move"] ?? "");
            case "vote":
            case "act":
                Phase expected = type == "vote" ? Phase.Vote : Phase.Combat;
                if (Current != expected || encounter is null) return "there is nothing to choose";
                if (type == "act" && seat.Character is null) return "claim a character first";
                int choice = (int?)action["choice"] ?? -1;
                if (choice < 0 || choice >= encounter.Options.Length) return "no such choice";
                choices[member] = choice;
                Changed();
                TryResolve(force: false);
                return null;
            case "resolve":
                if (member != Leader) return "only the leader decides without the others";
                if (Current is not (Phase.Vote or Phase.Combat)) return "nothing is waiting";
                TryResolve(force: true);
                return null;
            case "continue":
                if (member != Leader) return "only the leader moves on";
                if (Current != Phase.Result) return "nothing to continue";
                Current = Phase.Explore;
                PhaseId++;
                Changed();
                return null;
            default:
                return $"unknown action {type}";
        }
    }

    private string? Move(string move)
    {
        (int dx, int dz) = Facing switch { 0 => (0, -1), 1 => (1, 0), 2 => (0, 1), _ => (-1, 0) };
        switch (move)
        {
            case "left": Facing = (Facing + 3) % 4; Changed(); return null;
            case "right": Facing = (Facing + 1) % 4; Changed(); return null;
            case "forward": break;
            case "back": dx = -dx; dz = -dz; break;
            default: return $"no move {move}";
        }
        if (Blocked(X + dx, Z + dz)) return "a wall blocks the way";
        X += dx;
        Z += dz;
        encounter = Encounters.FirstOrDefault(e => e.X == X && e.Z == Z && !resolvedTiles.Contains((e.X, e.Z)));
        if (encounter is not null)
        {
            Current = encounter.Kind;
            PhaseId++;
            choices.Clear();
            Note(encounter.Prompt);
        }
        Changed();
        return null;
    }

    /// <summary>
    /// Who must choose: every member (with a character, in combat), present or
    /// not. The party waits for an absent member until the leader decides
    /// without them; nothing times out.
    /// </summary>
    private IEnumerable<uint> Deciders() => seats
        .Where(seat => Current != Phase.Combat || seat.Value.Character is not null)
        .Select(seat => seat.Key);

    private void TryResolve(bool force)
    {
        if (encounter is null || Current is not (Phase.Vote or Phase.Combat)) return;
        List<uint> waiting = Deciders().Where(member => !choices.ContainsKey(member)).ToList();
        if (waiting.Count > 0 && !force) return;
        string outcome;
        if (Current == Phase.Vote)
        {
            // Majority of the votes cast; the leader's vote breaks a tie.
            var tally = choices.Values.GroupBy(choice => choice).Select(group => (Choice: group.Key, Count: group.Count())).ToList();
            int best = tally.Count == 0 ? 0 : tally.Max(entry => entry.Count);
            List<int> top = tally.Where(entry => entry.Count == best).Select(entry => entry.Choice).ToList();
            int decided = top.Count == 1 ? top[0]
                : choices.TryGetValue(Leader, out int leaderChoice) && top.Contains(leaderChoice) ? leaderChoice
                : top.DefaultIfEmpty(0).Min();
            string how = top.Count > 1 ? " (the leader broke the tie)" : "";
            outcome = $"The party chose {encounter.Options[decided]}{how}: {string.Join(", ", tally.Select(entry => $"{encounter.Options[entry.Choice]} {entry.Count}"))}.";
        }
        else
        {
            IEnumerable<string> acts = choices.Select(choice => $"{seats[choice.Key].Character} {encounter.Options[choice.Value].ToLowerInvariant()}s");
            int attackers = choices.Values.Count(choice => choice == 0);
            outcome = $"{string.Join(", ", acts)}. " + (attackers > 0 ? "The goblins are driven off." : "The goblins slink away.");
        }
        if (waiting.Count > 0)
            outcome += $" ({string.Join(", ", waiting.Select(member => seats[member].Name))} did not choose.)";
        resolvedTiles.Add((encounter.X, encounter.Z));
        encounter = null;
        choices.Clear();
        Current = Phase.Result;
        PhaseId++;
        Note(outcome);
    }

    private void Note(string line)
    {
        log.Add(line);
        if (log.Count > LogLines) log.RemoveAt(0);
        Changed();
    }

    private void Changed() => Revision++;

    /// <summary>The whole state every member renders from.</summary>
    public JsonObject Snapshot() => new()
    {
        ["kind"] = "state",
        ["revision"] = Revision,
        ["phaseId"] = PhaseId,
        ["phase"] = Current.ToString().ToLowerInvariant(),
        ["x"] = X,
        ["z"] = Z,
        ["facing"] = Facing,
        ["leader"] = Leader,
        ["prompt"] = encounter?.Prompt ?? "",
        ["options"] = new JsonArray((encounter?.Options ?? []).Select(option => (JsonNode)option).ToArray()),
        ["waiting"] = new JsonArray(Deciders().Where(member => encounter is not null && !choices.ContainsKey(member)).Select(member => (JsonNode)member).ToArray()),
        ["chosen"] = new JsonArray(choices.Keys.Select(member => (JsonNode)member).ToArray()),
        ["seats"] = new JsonArray(seats.Select(seat => (JsonNode)new JsonObject
        {
            ["member"] = seat.Key,
            ["name"] = seat.Value.Name,
            ["character"] = seat.Value.Character ?? "",
            ["present"] = seat.Value.Connected,
        }).ToArray()),
        ["log"] = new JsonArray(log.Select(line => (JsonNode)line).ToArray()),
    };

    private sealed class Seat(string name)
    {
        public string Name { get; set; } = name;
        public string? Character { get; set; }
        public bool Connected { get; set; }
    }

    private sealed record Encounter(int X, int Z, Phase Kind, string Prompt, string[] Options);
}

internal enum Phase
{
    Explore,
    Vote,
    Combat,
    Result,
}
