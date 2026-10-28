using System.Numerics;

namespace CsharpCloakComparison;

/// <summary>One stretch of the run: the clip, ground speed (m/s), how far it turns over the stretch (radians), and whether it jumps or the wind blows.</summary>
internal sealed record Phase(string Name, float Seconds, string Clip, float Speed, float Turn = 0, bool Jump = false, bool Windy = false, bool Brush = false);

/// <summary>
/// The scripted run both modes repeat: idle, walk, run, an abrupt turn and
/// stop, a jump, a gust of wind and a walk past a pillar that brushes the
/// cloak. It advances by fixed steps, so a time of the run is the same pose
/// and place in every mode.
/// </summary>
internal static class Sequence
{
    private const float Walk = 1.4f, Run = 3.6f;
    public const string Idle = "idle", Walking = "walk", Running = "jog";
    public const string JumpStart = "jump_start", JumpLoop = "jump_loop", JumpLand = "jump_land";
    /// <summary>The jump: its take-off, flight and landing within its phase (s), and its apex (m).</summary>
    public const float TakeOff = .25f, Landing = .95f, Apex = .55f;
    /// <summary>How long clips cross-fade at a phase change (s).</summary>
    public const float Fade = .2f;

    /// <summary>The cloak is attached in the rest pose (as it was derived); the settle phase eases into idle over this long (s) and lets it hang.</summary>
    public const float SettleBlend = 1;

    public static readonly Phase[] Phases =
    [
        new("settle", 2, Idle, 0),
        new("idle", 2, Idle, 0),
        new("walk", 3, Walking, Walk),
        new("run", 3, Running, Run),
        new("turn", .3f, Running, Run, Turn: MathF.PI),
        new("run back", 2, Running, Run),
        new("stop", 2, Idle, 0),
        new("jump", 1.2f, JumpStart, 0, Jump: true),
        new("landed", 1.5f, Idle, 0),
        new("wind", 4, Idle, 0, Windy: true),
        new("brush", 4, Walking, Walk, Brush: true),
        new("rest", 2, Idle, 0),
    ];

    public static float Duration => Phases.Sum(phase => phase.Seconds);

    /// <summary>The phase at <paramref name="seconds"/> into the run and how far into it.</summary>
    public static (Phase Phase, int Index, float Into) At(float seconds)
    {
        for (int index = 0; index < Phases.Length; index++)
        {
            if (seconds < Phases[index].Seconds || index == Phases.Length - 1)
            {
                return (Phases[index], index, MathF.Min(seconds, Phases[index].Seconds));
            }
            seconds -= Phases[index].Seconds;
        }
        throw new InvalidOperationException("no phases");
    }

    /// <summary>
    /// The clip a phase plays <paramref name="into"/> seconds in (<paramref name="seconds"/>
    /// into the run), and that clip's own time: looping clips run on the run's
    /// clock, so one carries on through phases that share it; the jump's clips
    /// start with their part of the jump.
    /// </summary>
    public static (string Clip, float Time) ClipAt(Phase phase, float into, float seconds) => phase.Jump switch
    {
        true when into < TakeOff => (JumpStart, into),
        true when into < Landing => (JumpLoop, into - TakeOff),
        true => (JumpLand, into - Landing),
        false => (phase.Clip, seconds),
    };

    /// <summary>Whether a clip loops (the jump's take-off and landing play once).</summary>
    public static bool Loops(string clip) => clip is not (JumpStart or JumpLand);

    /// <summary>How much of a phase's clip is laid over the rest pose <paramref name="into"/> it: the settle phase eases in.</summary>
    public static float Weight(Phase phase, float into)
    {
        if (phase != Phases[0]) return 1;
        float along = Math.Clamp(into / SettleBlend, 0, 1);
        return along * along * (3 - 2 * along);
    }

    /// <summary>How high a jump is <paramref name="into"/> its phase: a parabola from take-off to landing.</summary>
    public static float Height(Phase phase, float into)
    {
        if (!phase.Jump || into <= TakeOff || into >= Landing) return 0;
        float along = (into - TakeOff) / (Landing - TakeOff);
        return 4 * Apex * along * (1 - along);
    }
}

/// <summary>Where a character is on the run: integrated step by step from the phases.</summary>
internal struct Runner
{
    public float Seconds;
    public Vector3 Position;
    public float Yaw;

    public Runner(Vector3 start) { Position = start; }

    public readonly (Phase Phase, int Index, float Into) Now => Sequence.At(Seconds);

    public readonly Vector3 Forward => new(MathF.Sin(Yaw), 0, MathF.Cos(Yaw));

    public void Advance(float dt)
    {
        (Phase phase, _, _) = Now;
        Yaw += phase.Turn / phase.Seconds * dt;
        Position += Forward * phase.Speed * dt;
        Seconds += dt;
    }

    public readonly Vector3 Root
    {
        get
        {
            (Phase phase, _, float into) = Now;
            return Position + Vector3.UnitY * Sequence.Height(phase, into);
        }
    }

    public readonly Quaternion Facing => Quaternion.CreateFromAxisAngle(Vector3.UnitY, Yaw);
}
