using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// First-person arms drawn by one rig profile (<see cref="ArmRig"/>): its model on the viewmodel layer, placed so its
/// right upper arm sits at the arms' shoulder, whose hands reach their weapons' grips by the Engine's two-bone IK each
/// update over the rig's clip, each hand turned with its weapon's look by the hold (its forearm's twist joints sharing
/// the roll, its wrist bending only as far as the rig's <see cref="ArmRig.Wrist"/> range) and its digits shaped and closed
/// as the look and motion say. Only the weapons move by authored motion; the
/// arms follow, elbows bending toward their poles, the left mirroring the right. Nothing names a rig's frames: each hand's frame and digit axes are measured from
/// the drawn model once, so any rig with two arms and five digits a hand can hold.
/// </summary>
/// <remarks>
/// Each update: set <see cref="Lean"/> and <see cref="Carry"/>, call <see cref="Aim"/> with both hands' aims, publish
/// <see cref="Fact"/> in the snapshot, then call <see cref="Reach"/> with the same aims once that snapshot is published.
/// Arms can instead be clip-led, by baked arm clips with the look riding the hand: call <see cref="Lead"/> and
/// <see cref="Ride"/> first (see <see cref="Lead"/>).
/// </remarks>
public sealed class FirstPersonArms : IDisposable
{
    // A reach never quite straightens the arm, and the body leans at most this far (metres).
    private const float Slack = .01f, MostReaching = .3f;
    private const float Radians = MathF.PI / 180;
    // A clamped hand's wrist is settled once a pass moves it less than this (metres), within at most so many passes.
    private const float Settled = 1e-5f;
    private const int MostSettling = 8;

    private readonly IEngineContext engine;
    private readonly HeldArms arms;
    private readonly ArmRig rig;
    private readonly RenderResource model;
    private readonly RenderResource[] packs;
    private readonly string[] clips;
    private readonly ClipLead lead = new();
    private readonly AnimationLayerSet layers = new();
    private bool layered;
    // The main hand as the clip draws it this update, once ridden: the aim it reaches by, and its wrist turned back into
    // range (model space) when the clip bends it past.
    private (HandAim Aim, (Vector3 Wrist, Vector3 Pole, Quaternion Canonical)? Clamped)? ride;
    // The clips' pose this update, once ridden: the shoulders where the clip puts them, for the reach's wrist range.
    private JointPose[]? sampled;
    private readonly Appearance appearance;
    private readonly Side main, off;
    private readonly HandRig mainRig, offRig;
    private AnimationInstance? instance;
    // Where each upper arm was last drawn (model space); the right one places the model so it sits at the shoulder.
    private Vector3? upperArm, offUpperArm;
    private Vector3 reaching;
    private JointPose[]? drawnJoints;
    // Which hand each two-bone chain of the last pose reached (off or not), and how far the arms were clip-led then.
    private bool[] reachedBy = [];
    private float drawnLed;

    /// <summary>
    /// Opens the rig's model and names its joints. Throws <see cref="InvalidOperationException"/> naming the field when
    /// the model lacks the rig's clip or a joint it names (and the Engine's own error when the model does not open).
    /// </summary>
    /// <param name="objectId">The published object the arms are drawn on; an object keeps one animation instance, so a new rig takes a new object.</param>
    public FirstPersonArms(IEngineContext engine, HeldArms arms, ArmRig rig, ulong objectId)
        : this(engine, arms, rig, objectId, engine.Content.OpenReference(new(rig.Model)), null, owned: true) { }

    /// <summary>
    /// Draws the rig with a model the product has already opened, such as one from a <see cref="ProductContentBundle"/>
    /// it loaded beside its own content; <see cref="ArmRig.Model"/> then only names the model in errors. The reference
    /// is read here and stays the caller's to dispose.
    /// </summary>
    /// <param name="objectId">The published object the arms are drawn on; an object keeps one animation instance, so a new rig takes a new object.</param>
    /// <param name="content">The arms model's content.</param>
    /// <param name="packs">The content of the rig's <see cref="ArmRig.Packs"/>, in order, when they are opened by the caller too; read here and the caller's to dispose.</param>
    public FirstPersonArms(IEngineContext engine, HeldArms arms, ArmRig rig, ulong objectId, ContentReference content, IReadOnlyList<ContentReference>? packs = null)
        : this(engine, arms, rig, objectId, content, packs, owned: false) { }

    private FirstPersonArms(IEngineContext engine, HeldArms arms, ArmRig rig, ulong objectId, ContentReference content, IReadOnlyList<ContentReference>? packContent, bool owned)
    {
        this.engine = engine;
        this.arms = arms;
        this.rig = rig;
        Object = objectId;
        if (packContent is { } given && given.Count != rig.Packs.Length)
            throw new InvalidOperationException($"packs: the rig names {rig.Packs.Length} clip packs and {given.Count} were given.");
        try { model = engine.Animation.OpenAnimatedMeshFromContent(new(content)); }
        finally { if (owned) content.Dispose(); }
        List<RenderResource> opened = [];
        try
        {
            // Packs are associated before the appearance is made, so its clips are the model's own and then each pack's.
            for (int i = 0; i < rig.Packs.Length; i++)
            {
                ArmClipPack pack = rig.Packs[i];
                if (packContent is { } open) opened.Add(engine.Animation.OpenAnimationClipPackFromContent(new(open[i])));
                else
                {
                    using ContentReference source = engine.Content.OpenReference(new(pack.Path));
                    opened.Add(engine.Animation.OpenAnimationClipPackFromContent(new(source)));
                }
                engine.Animation.AssociateAnimationClipPack(new(model, opened[^1], pack.Producer, pack.License));
            }
            packs = [.. opened];
            AnimationJointInfo[] joints = engine.Animation.ReadJoints(model).ToArray();
            clips = [.. engine.Animation.ReadClips(model).ToArray().Select(c => c.Id)];
            if (!clips.Contains(rig.Clip))
                throw new InvalidOperationException($"clip: the arms model '{rig.Model}' has no clip '{rig.Clip}'. Its clips: {string.Join(", ", clips)}.");
            uint Joint(string name, string field)
            {
                int at = Array.FindIndex(joints, j => j.Id == name);
                return at >= 0 ? (uint)at : throw new InvalidOperationException($"{field}: the arms model '{rig.Model}' has no joint '{name}'.");
            }
            bool Descends(uint joint, uint from)
            {
                for (uint at = joints[joint].Parent; at < joints.Length; at = joints[at].Parent)
                    if (at == from) return true;
                return false;
            }
            Side Arm(ArmJoints chain, string field, bool left)
            {
                uint upper = Joint(chain.Upper, $"{field}.upper"), hand = Joint(chain.Hand, $"{field}.hand");
                (uint, float)[] twists = [.. chain.Twist.Select((t, i) =>
                {
                    uint joint = Joint(t.Joint, $"{field}.twist[{i}]");
                    // A twist joint turns about the forearm with its descendants; under the hand it would turn the hand's own.
                    if (!Descends(joint, upper) || joint == hand || Descends(joint, hand))
                        throw new InvalidOperationException($"{field}.twist[{i}]: '{t.Joint}' in the arms model '{rig.Model}' does not lie on the arm below '{chain.Upper}' and above '{chain.Hand}'.");
                    return (joint, t.Share);
                })];
                return new(left, upper, Joint(chain.Lower, $"{field}.lower"), hand,
                    [.. chain.Fingers.Select((f, i) => f.Select(n => Joint(n, $"{field}.fingers[{i}]")).ToArray())],
                    [.. chain.Thumb.Select(n => Joint(n, $"{field}.thumb"))], twists);
            }
            main = Arm(rig.Main, "main", false);
            off = Arm(rig.Off, "off", true);
            // Each hand's joints named once for the Engine's hand landmarks.
            HandRig Hand(ArmJoints chain, HandSide side) => HandRig.FromJoints(joints, objectId, side, chain.Hand, chain.Thumb,
                chain.Fingers[0], chain.Fingers[1], chain.Fingers[2], chain.Fingers[3]);
            mainRig = Hand(rig.Main, HandSide.Right);
            offRig = Hand(rig.Off, HandSide.Left);
            appearance = engine.Animation.CreateAnimatedMeshAppearance(new(model));
        }
        catch
        {
            foreach (RenderResource pack in opened) pack.Dispose();
            model.Dispose();
            throw;
        }
    }

    /// <summary>The published object these arms are drawn on.</summary>
    public ulong Object { get; }

    /// <summary>The profile drawing these arms.</summary>
    public ArmRig Rig => rig;

    /// <summary>Each hand's measured turn from its canonical frame to its joint's (degrees about X, Y, Z), once measured; null before.</summary>
    public (Vector3 Main, Vector3 Off)? HandFrames => main.Measured ? (HeldFrames.Degrees(main.Frame), HeldFrames.Degrees(off.Frame)) : null;

    /// <summary>
    /// A hand's knuckle angles (<see cref="HandKnuckles"/>): on the rig's clip alone, as first drawn, and as last drawn
    /// (with the rig's grip, the look's shape and the closure); null before the hands are measured.
    /// </summary>
    public (HandShape Clip, HandShape Drawn)? Knuckles(bool offHand) =>
        (offHand ? off : main) is { Clip: { } clip } side && drawnJoints is { } joints ? (clip, side.Knuckles(joints)) : null;

    /// <summary>A hand's rig joints for the Engine's hand landmarks: the main (right) hand or the off (left).</summary>
    public HandRig Hand(bool offHand) => offHand ? offRig : mainRig;

    /// <summary>How far the motion leans the body: the shoulders' move from where the arms place them (camera space).</summary>
    public Vector3 Lean { get; set; }

    /// <summary>
    /// How procedural layers move the arms and what they hold: head bob, sway, landing and settle, and
    /// <see cref="HeldLayers"/> punches (camera space metres), turning them by <see cref="CarryTurn"/> about the shoulder.
    /// Clip-led arms carry their ridden look with them; weapon-led hands keep their grips, so give a weapon-led look the
    /// same offset as today and the two modes move alike.
    /// </summary>
    public Vector3 Carry { get; set; }

    /// <summary>How procedural layers turn the arms about the shoulder (camera space), with <see cref="Carry"/>.</summary>
    public Quaternion CarryTurn { get; set; } = Quaternion.Identity;

    /// <summary>The clips the arms can play when clip-led: the model's own, then each of the rig's packs'.</summary>
    public IReadOnlyList<string> Clips => clips;

    /// <summary>What clip-led arms are sampling: the clip, its time and phase, any clip fading out under it, and how far the arms are clip-led.</summary>
    public ArmClipReading Clip => lead.Reading;

    /// <summary>How far the body leans in on its own so a hand reaches a grip past its arm's length (camera space).</summary>
    public Vector3 Reaching => reaching;

    /// <summary>
    /// How far each wrist was drawn from its target, as the Engine's solve reports it (metres): more than a centimetre or
    /// so is a grip out of the arm's reach.
    /// </summary>
    public (float Main, float Off) Shortfall { get; private set; }

    /// <summary>
    /// How each wrist was asked to bend from its forearm and how far its hand was turned back to keep it in the rig's
    /// <see cref="ArmRig.Wrist"/> range on the last reach: a look whose hold asks past the range shows here.
    /// </summary>
    public (WristBend Main, WristBend Off) WristClamp => (main.Bend, off.Bend);

    /// <summary>Each arm's length from the upper arm joint through the elbow to the hand joint as last drawn (metres), once measured.</summary>
    public (float Main, float Off) ArmLength { get; private set; }

    /// <summary>
    /// Before the arms are published: leans the body in, beyond the motion's own <see cref="Lean"/>, when a grip lies
    /// further from its shoulder than the arm reaches, so both hands stay on their grips whatever the rig's arm length.
    /// The arm furthest short sets the lean; the other arm's IK takes up the move.
    /// </summary>
    public void Aim(HandAim mainHand, HandAim offHand)
    {
        reaching = Vector3.Zero;
        if (upperArm is not { } right || offUpperArm is not { } left || ArmLength.Main <= 0) return;
        Transform placed = Placement();
        Vector3 Shoulder(Vector3 joint) => placed.Translation + Vector3.Transform(joint * rig.Scale, placed.Rotation);
        Vector3 best = Vector3.Zero;
        foreach (var (shoulder, hand, length, leftArm) in new[] { (Shoulder(right), mainHand, ArmLength.Main, false), (Shoulder(left), offHand, ArmLength.Off, true) })
        {
            Vector3 toward = Wrist(hand, leftArm).Wrist - shoulder;
            float beyond = toward.Length() - (length - Slack);
            if (beyond > best.Length()) best = Vector3.Normalize(toward) * beyond;
        }
        reaching = best.Length() > MostReaching ? Vector3.Normalize(best) * MostReaching : best;
    }

    /// <summary>
    /// First each update: whether the arms are led by a clip, and which clip at what time (<see cref="ArmClip.At"/> fits an
    /// action's), or by their weapons (null), as they are until called. Clip-led arms play the clip on both arms with
    /// no reach, the look rides the main hand (<see cref="Ride"/>), and the off hand reaches its grip by its aim's
    /// <see cref="HandAim.Reach"/>. A new clip fades in over the one before by its frame's fade, and the arms fade
    /// between clip-led and weapon-led over the same seconds; <paramref name="delta"/> is the seconds since the last
    /// update, on the clock the action's time is on. Then call <see cref="Ride"/>, <see cref="Fact"/> and
    /// <see cref="Reach"/> (clip-led arms skip <see cref="Aim"/>). Throws <see cref="InvalidOperationException"/> for a
    /// clip the arms do not have (<see cref="Clips"/>).
    /// </summary>
    public void Lead(ArmClipFrame? frame, float delta)
    {
        if (frame is { } f && !clips.Contains(f.Clip))
            throw new InvalidOperationException($"clip: the arms model '{rig.Model}' and its packs have no clip '{f.Clip}'. Its clips: {string.Join(", ", clips)}.");
        lead.Advance(frame, delta);
        (ride, sampled) = (null, null);
        if (lead.Led > 0) reaching = Vector3.Zero;
    }

    /// <summary>
    /// After <see cref="Lead"/>, before the arms are published: where the look is held this update (camera space), its
    /// grip point and turn. On clip-led arms it rides the main hand as the clip draws it, held by the aim's solved grip,
    /// hold and the rig's palm as a weapon-led hand holds its look, the wrist kept within the rig's range; publish the
    /// look there, and place a look held in both hands' off grip from it. Weapon-led, it is the aim's own grip and look;
    /// fading between the two, it blends them. Give <see cref="Reach"/> the same main aim.
    /// </summary>
    public (Vector3 Grip, Quaternion Look) Ride(HandAim mainHand)
    {
        (ride, sampled) = (null, null);
        if (lead.Led <= 0 || lead.Playing is not { } playing) return (mainHand.Grip, mainHand.Look);
        Prepare();
        JointPose[] posed = engine.Animation.SampleModelJointPose(lead.From is { } from
            ? new(model, from.Clip, from.Time, playing.Clip, playing.Time, lead.Blend, true, Unplaced)
            : AnimationModelPoseSampleRequest.OfClip(model, playing.Clip, playing.Time, Unplaced)).ToArray();
        Transform placed = Placement();
        Vector3 shoulder = posed[main.Upper].Model.Translation, wrist = posed[main.Hand].Model.Translation, elbow = posed[main.Lower].Model.Translation;
        // The clip's elbow is the pole it bends toward, unless the clip holds the arm straight: then the aim's pole.
        Vector3 line = Vector3.Normalize(wrist - shoulder), bent = elbow - shoulder - line * Vector3.Dot(elbow - shoulder, line);
        if (bent.Length() < 1e-3f * Vector3.Distance(shoulder, wrist))
            elbow = Vector3.Transform(mainHand.Pole - placed.Translation, Quaternion.Inverse(placed.Rotation)) / rig.Scale;
        Quaternion canonical = Quaternion.Normalize(posed[main.Hand].Model.Rotation * Quaternion.Inverse(main.Frame));
        (Quaternion kept, Vector3 pole, WristBend bend) = main.Limit(posed, rig.Wrist, wrist, elbow, canonical);
        main.Bend = bend;
        Vector3 Out(Vector3 point) => placed.Translation + Vector3.Transform(point * rig.Scale, placed.Rotation);
        Quaternion turn = Quaternion.Normalize(placed.Rotation * kept);
        (Quaternion hold, Vector3 palm) = Grasp(mainHand, false);
        Vector3 grip = Out(wrist) + Vector3.Transform(palm, turn);
        Quaternion look = Quaternion.Normalize(turn * Quaternion.Inverse(hold));
        float led = lead.Led;
        HandAim aim = mainHand with
        {
            Grip = Vector3.Lerp(mainHand.Grip, grip, led),
            Look = Quaternion.Slerp(mainHand.Look, look, led),
            Pole = Vector3.Lerp(mainHand.Pole, Out(pole), led),
        };
        ride = (aim, bend.Clamped > 0 ? (wrist, pole, kept) : null);
        sampled = posed;
        return (aim.Grip, aim.Look);
    }

    /// <summary>The arms' fact for the snapshot: the model placed so its right upper arm sits at the shoulder, facing -Z, on the viewmodel layer.</summary>
    public AppearanceFact Fact(bool shown) => new(Object, false, 0, Placement(), appearance, shown, RenderLayer.Viewmodel);

    /// <summary>
    /// Once the arms are in the published snapshot, reaches each hand for its grip with the hand turned with its look,
    /// its digits shaped as the look says and closed as given. The off hand's grip and turn are already mirrored.
    /// Clip-led arms play their clip: the main hand rides it (turned back into the wrist's range where the clip bends it
    /// past), and the off hand reaches by its aim's <see cref="HandAim.Reach"/>.
    /// </summary>
    public void Reach(HandAim mainHand, HandAim offHand)
    {
        if (instance is null)
        {
            instance = engine.Animation.CreateInstance(new(appearance, Object));
            engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, rig.Clip, AnimationLoopMode.Repeat, 1, 1, true, 0, true, 0));
        }
        if (engine.Animation.ReadJointPose(instance) is { Reported: true } drawn)
        {
            JointPose[] joints = drawn.Joints.ToArray();
            drawnJoints = joints;
            // A clip moves the shoulders as it acts, so the arms stay placed where they last were weapon-led.
            if (drawnLed <= 0 || upperArm is null)
            {
                upperArm = joints[main.Upper].Model.Translation;
                offUpperArm = joints[off.Upper].Model.Translation;
            }
            if (!main.Measured) main.Measure(joints, rig.Digits);
            if (!off.Measured) off.Measure(joints, rig.Digits);
            ArmLength = (main.Length(joints) * rig.Scale, off.Length(joints) * rig.Scale);
            IkChainReach[] reached = drawn.IkChains.ToArray();
            float Fell(bool offHand) => Array.IndexOf(reachedBy, offHand) is var at && at >= 0 && at < reached.Length ? reached[at].Shortfall * rig.Scale : 0;
            Shortfall = (Fell(false), Fell(true));
        }
        drawnLed = lead.Led;
        Layer();
        Transform placed = Placement();
        Quaternion unturn = Quaternion.Inverse(placed.Rotation);
        // Camera space into the model's own space, where the IK targets and hand turns are given.
        Vector3 Into(Vector3 point) => Vector3.Transform(point - placed.Translation, unturn) / rig.Scale;
        List<TwoBoneIk> chains = [];
        List<bool> reachers = [];
        List<JointOverride> overrides = [];
        List<JointTwist> twists = [];
        float led = lead.Led;
        foreach (var (side, aimed) in new[] { (main, mainHand), (off, offHand) })
        {
            HandAim hand = aimed;
            // The forearm's twist joints share the hand's roll about the forearm, as the Engine draws it after the reach.
            twists.AddRange(side.Twists.Select(t => new JointTwist(t.Joint, side.Lower, side.Hand, t.Share)));
            // How far the hand leaves its clip for its aim, and its digits their clip for the look's shape.
            float weight = 1;
            if (led > 0 && side == main && ride is { } riding)
            {
                if (led >= 1)
                {
                    // Fully clip-led, the hand is the clip's, unless the clip bends its wrist past the range.
                    if (riding.Clamped is { } clamped && side.Measured)
                    {
                        chains.Add(new(side.Upper, side.Lower, side.Hand, PoseSpace.Model, clamped.Wrist, clamped.Pole, 1));
                        reachers.Add(false);
                        overrides.Add(JointOverride.Orient(side.Hand, PoseSpace.Model, Quaternion.Normalize(clamped.Canonical * side.Frame)));
                    }
                    continue;
                }
                hand = riding.Aim;
            }
            else if (led > 0 && side == off)
            {
                weight = 1 + (Math.Clamp(hand.Reach, 0, 1) - 1) * led;
                if (weight <= 0) continue;
            }
            float shaping = side == main ? 1 - led : weight;
            (Quaternion turn, Vector3 palm) = Holding(hand, side.Left);
            Vector3 grip = Into(hand.Grip), pole = Into(hand.Pole);
            palm /= rig.Scale;
            // The hand's canonical frame in the model's space, kept within the wrist's range of the forearm the reach
            // draws. A hand turned back keeps its palm on the grip, so its wrist moves and the forearm with it: the range
            // is taken again from the forearm that reaches there until the wrist settles, and the reach is solved to
            // the wrist the range was last taken at.
            Quaternion canonical = Quaternion.Normalize(unturn * turn);
            Vector3 target = grip - Vector3.Transform(palm, canonical);
            if (side.Measured && (sampled ?? drawnJoints) is { } posed)
            {
                (Quaternion kept, Vector3 swung, WristBend bend) = side.Limit(posed, rig.Wrist, target, pole, canonical);
                WristBend asked = bend;
                for (int i = 0; i < MostSettling && bend.Clamped > 0; i++)
                {
                    Vector3 next = grip - Vector3.Transform(palm, kept);
                    if (Vector3.Distance(next, target) * rig.Scale < Settled) break;
                    target = next;
                    (kept, swung, bend) = side.Limit(posed, rig.Wrist, target, pole, canonical);
                }
                (canonical, pole, side.Bend) = (kept, swung, bend with { Flexion = asked.Flexion, Ulnar = asked.Ulnar, Roll = asked.Roll });
            }
            chains.Add(new(side.Upper, side.Lower, side.Hand, PoseSpace.Model, target, pole, weight));
            reachers.Add(side == off);
            if (!side.Measured) continue;
            overrides.Add(JointOverride.Orient(side.Hand, PoseSpace.Model, Quaternion.Normalize(canonical * side.Frame), weight));
            if (shaping > 0)
                overrides.AddRange(side.Close(rig, arms.Hands, hand.Shape ?? hand.Solved?.Shape, hand.Closure).Select(t => JointOverride.AddLocalRotation(t.Joint, t.Turn, shaping)));
        }
        reachedBy = [.. reachers];
        engine.Animation.SetPose(new(instance, chains.ToArray(), overrides.ToArray(), twists.ToArray()));
    }

    // The clips over the rig's: the one fading out at its held time, then the playing one over it, as far as the arms
    // are clip-led. Ride samples the same blend. Weapon-led arms lay none.
    private void Layer()
    {
        if (instance is null || (lead.Led <= 0 && !layered)) return;
        layers.Clear();
        if (lead.Led > 0 && lead.Playing is { } playing)
        {
            if (lead.From is { } from) layers.Override((uint)Array.IndexOf(clips, from.Clip), from.Time, lead.Led);
            layers.Override((uint)Array.IndexOf(clips, playing.Clip), playing.Time, lead.Led * (lead.From is null ? 1 : lead.Blend));
        }
        engine.Animation.SetLayers(layers.ToRequest(instance));
        layered = layers.Count > 0;
    }

    /// <summary>
    /// Where a hand's wrist goes to hold its grip (camera space), and how the hand's canonical frame turns: with the look,
    /// by its solved grip, else the look's own hold or else the arms', with the palm, not the wrist, on the grip. The
    /// left hand holds as the right does, mirrored. This is the hand as asked, before the wrist's range.
    /// </summary>
    public (Vector3 Wrist, Quaternion Turn) Wrist(HandAim hand, bool left)
    {
        (Quaternion turn, Vector3 palm) = Holding(hand, left);
        return (hand.Grip - Vector3.Transform(palm, turn), turn);
    }

    // How a hand's canonical frame turns to hold its grip (camera space), and the grip's offset from its hand joint in
    // that frame (metres).
    private (Quaternion Turn, Vector3 Palm) Holding(HandAim hand, bool left)
    {
        (Quaternion hold, Vector3 palm) = Grasp(hand, left);
        return (Quaternion.Normalize(hand.Look * hold), palm);
    }

    // How a hand's canonical frame turns from its look's, and the grip's offset from its hand joint in that frame (metres).
    private (Quaternion Hold, Vector3 Palm) Grasp(HandAim hand, bool left)
    {
        Quaternion hold = hand.Solved?.Hold ?? HeldFrames.Turn(hand.Hold ?? arms.Hold);
        Vector3 palm = hand.Solved?.Palm ?? rig.Palm;
        // The left hand's frame mirrors the right's, so its palm offset is the right's mirrored; its hold is the right
        // hand's mirrored too when it holds its own (mirrored) look, but given as is on a look held in both hands.
        if (left) palm = HeldFrames.Mirror(palm);
        if (left && hand.HoldMirrored) hold = HeldFrames.Mirror(hold);
        return (hold, palm);
    }

    private static Transform Unplaced => new(Vector3.Zero, Quaternion.Identity, Vector3.One);

    // Clip-led arms ride a hand before any pose is drawn: the hands are measured, and the upper arm placed, from the
    // rig's clip at its start, as a first weapon-led reach would measure them.
    private void Prepare()
    {
        if (main.Measured && off.Measured && upperArm is not null) return;
        JointPose[] joints = engine.Animation.SampleModelJointPose(AnimationModelPoseSampleRequest.OfClip(model, rig.Clip, 0, Unplaced)).ToArray();
        upperArm ??= joints[main.Upper].Model.Translation;
        offUpperArm ??= joints[off.Upper].Model.Translation;
        if (!main.Measured) main.Measure(joints, rig.Digits);
        if (!off.Measured) off.Measure(joints, rig.Digits);
    }

    public void Dispose()
    {
        instance?.Dispose();
        appearance.Dispose();
        model.Dispose();
        foreach (RenderResource pack in packs) pack.Dispose();
    }

    // The model turned and scaled into camera space, its right upper arm (as last drawn weapon-led) at the arms'
    // shoulder, carried and turned about it by the procedural layers.
    private Transform Placement()
    {
        Quaternion turn = Quaternion.Normalize(CarryTurn * HeldFrames.Turn(rig.Turn));
        Vector3 at = arms.Shoulder + Lean + reaching + Carry - Vector3.Transform((upperArm ?? Vector3.Zero) * rig.Scale, turn);
        return new(at, turn, new Vector3(rig.Scale));
    }

    // One arm: its IK chain, its digit joints (index, middle, ring and little fingers, then the thumb), its forearm
    // twist joints and their shares, and what is measured from the drawn model: the turn from the hand's canonical
    // frame to its joint's, each finger joint's curl axis in its own frame, and each thumb joint's turn in the hand's
    // canonical frame and its bone in its own frame.
    private sealed class Side(bool left, uint upper, uint lower, uint hand, uint[][] fingers, uint[] thumb, (uint Joint, float Share)[] twists)
    {
        private readonly Dictionary<uint, Vector3> curls = [];
        private readonly (Quaternion Turn, Vector3 Bone)[] thumbs = new (Quaternion, Vector3)[3];

        internal bool Left => left;
        internal uint Upper => upper;
        internal uint Lower => lower;
        internal uint Hand => hand;
        internal (uint Joint, float Share)[] Twists => twists;
        internal bool Measured { get; private set; }
        internal Quaternion Frame { get; private set; } = Quaternion.Identity;
        internal HandShape? Clip { get; private set; }
        internal WristBend Bend { get; set; }

        internal float Length(JointPose[] joints) =>
            Vector3.Distance(joints[upper].Model.Translation, joints[lower].Model.Translation)
            + Vector3.Distance(joints[lower].Model.Translation, joints[hand].Model.Translation);

        // The knuckles are fixed to the hand, so where they sit in its frame gives the canonical frame whatever the
        // hand's turn; each finger's curl axis is taken in the joint's own frame, so it holds however the finger has
        // turned. A thumb's bones are taken as HandKnuckles reads them, so it can be aimed where a shape reads.
        internal void Measure(JointPose[] joints, DigitFrames frames)
        {
            Transform wrist = joints[hand].Model;
            Quaternion unturn = Quaternion.Inverse(wrist.Rotation);
            Vector3 Local(uint joint) => Vector3.Transform(joints[joint].Model.Translation - wrist.Translation, unturn);
            Frame = HandFrame.Measure(Local(fingers[0][0]), Local(fingers[1][0]), Local(fingers[3][0]), left);
            Quaternion canonical = Quaternion.Normalize(wrist.Rotation * Quaternion.Inverse(Frame));
            Vector3 x = Vector3.Transform(Vector3.UnitX, canonical);
            foreach (uint[] finger in fingers)
                foreach (uint joint in finger)
                    curls[joint] = frames == DigitFrames.Native ? Vector3.UnitX : Vector3.Transform(x, Quaternion.Inverse(joints[joint].Model.Rotation));
            Quaternion uncanonical = Quaternion.Inverse(canonical);
            for (int i = 0; i < 3; i++)
            {
                // The last joint's bone runs as the one before it does in that joint's own frame.
                (uint from, uint to) = i < 2 ? (thumb[i], thumb[i + 1]) : (thumb[1], thumb[2]);
                Vector3 bone = Vector3.Transform(joints[to].Model.Translation - joints[from].Model.Translation, Quaternion.Inverse(joints[from].Model.Rotation));
                thumbs[i] = (Quaternion.Normalize(uncanonical * joints[thumb[i]].Model.Rotation), Vector3.Normalize(bone));
            }
            Clip = Knuckles(joints);
            Measured = true;
        }

        // The hand's canonical frame (model space) kept within the wrist's range of the forearm the Engine's two-bone
        // solve will draw for this target and pole, and the pole to solve with. The forearm's line and the elbow's bend
        // follow from the shoulder, the bone lengths, the target and the pole as the solve finds them. The hand bends from
        // the forearm in its own frame; it rolls about the forearm from its neutral turn, the thumb toward the inside of
        // the elbow (away from its pole) and the back of the hand outward. A hand past the range first has its elbow
        // swing about the line from the shoulder to the target, up to the range's elbow, to bring the forearm under it;
        // then the bend is clamped to the range's edge and the roll back within it.
        internal (Quaternion Canonical, Vector3 Pole, WristBend Bend) Limit(JointPose[] joints, WristRange range, Vector3 target, Vector3 pole, Quaternion canonical)
        {
            const float Degenerate = 1e-6f, Margin = 1e-4f;
            Vector3 a = joints[upper].Model.Translation, elbow = joints[lower].Model.Translation;
            float l1 = Vector3.Distance(a, elbow), l2 = Vector3.Distance(elbow, joints[hand].Model.Translation), reach = Vector3.Distance(a, target);
            if (l1 < Degenerate || l2 < Degenerate || reach < Degenerate) return (canonical, pole, default);
            Vector3 along = (target - a) / reach;
            float span = l1 + l2, distance = Math.Clamp(reach, MathF.Abs(l1 - l2) + span * Margin, span * (1 - Margin));
            float cos = Math.Clamp((l1 * l1 + distance * distance - l2 * l2) / (2 * l1 * distance), -1, 1), sin = MathF.Sqrt(MathF.Max(0, 1 - cos * cos));
            Vector3 OffAxis(Vector3 point) => point - a - along * Vector3.Dot(point - a, along);
            Vector3 bend = OffAxis(pole) is var p && p.Length() > Degenerate ? Vector3.Normalize(p)
                : OffAxis(elbow) is var e && e.Length() > Degenerate ? Vector3.Normalize(e) : Vector3.Normalize(Vector3.Cross(along, MathF.Abs(along.X) < .9f ? Vector3.UnitX : Vector3.UnitY));
            Vector3 end = a + along * distance;
            // The elbow's bend toward the pole once swung, and the forearm's line from the elbow to the wrist.
            (Vector3 Bend, Vector3 Forearm) Arm(float swing)
            {
                Vector3 b = Vector3.Transform(bend, Quaternion.CreateFromAxisAngle(along, swing * Radians));
                return (b, Vector3.Normalize(end - (a + (along * cos + b * sin) * l1)));
            }
            // The forearm's line in the hand's frame lies along +Y with the wrist straight, toward the back (+Z) when the
            // hand flexes and toward the little finger's side when it bends toward the thumb. The roll is the thumb's turn
            // about the forearm from the inside of the elbow, toward the back of the hand (supination on either hand).
            float sideways = left ? -1 : 1;
            Vector3 thumb = new(-sideways, 0, 0);
            (float Flexion, float Ulnar, float Roll) Angles(Quaternion hand, (Vector3 Bend, Vector3 Forearm) arm)
            {
                Vector3 f = Vector3.Transform(arm.Forearm, Quaternion.Inverse(hand));
                Vector3 inside = -arm.Bend - arm.Forearm * Vector3.Dot(-arm.Bend, arm.Forearm), toward = Vector3.Transform(thumb, hand);
                toward -= arm.Forearm * Vector3.Dot(toward, arm.Forearm);
                float roll = inside.Length() > Degenerate && toward.Length() > Degenerate
                    ? MathF.Atan2(Vector3.Dot(Vector3.Cross(inside, toward), arm.Forearm) * sideways, Vector3.Dot(inside, toward)) / Radians : 0;
                return (MathF.Atan2(f.Z, f.Y) / Radians, -MathF.Asin(Math.Clamp(f.X * sideways, -1, 1)) / Radians, roll);
            }
            float Past((float Flexion, float Ulnar, float Roll) bent) =>
                new Vector3(MathF.Max(0, bent.Flexion - range.Flexion) + MathF.Max(0, -bent.Flexion - range.Extension),
                    MathF.Max(0, bent.Ulnar - range.Ulnar) + MathF.Max(0, -bent.Ulnar - range.Radial), MathF.Max(0, MathF.Abs(bent.Roll) - range.Roll)).Length();
            float PastAt(float swing) => Past(Angles(canonical, Arm(swing)));
            (float Flexion, float Ulnar, float Roll) asked = Angles(canonical, Arm(0));
            float swung = 0;
            if (Past(asked) > 0 && range.Elbow > 0)
            {
                // The least swing either way that brings the hand into range, refined between steps so the elbow moves
                // smoothly as the hand does; else the swing that leaves it least past, when that gains a worthwhile turn.
                const int Steps = 12;
                const float Worthwhile = .5f;
                float least = Past(asked);
                for (int i = 1; i <= Steps && least > 0; i++)
                    foreach (float sign in new[] { 1f, -1f })
                    {
                        float at = sign * range.Elbow * i / Steps, past = PastAt(at);
                        if (past <= 0)
                        {
                            float inside = at, outside = sign * range.Elbow * (i - 1) / Steps;
                            for (int r = 0; r < 8; r++)
                            {
                                float mid = (inside + outside) / 2;
                                if (PastAt(mid) <= 0) inside = mid; else outside = mid;
                            }
                            (swung, least) = (inside, 0);
                            break;
                        }
                        if (past < least - (swung == 0 ? Worthwhile : 0)) (swung, least) = (at, past);
                    }
            }
            var arm = Arm(swung);
            // The bend clamped in the hand's frame: the hand turned the least that brings the forearm's line there.
            (float flexion, float ulnar, _) = Angles(canonical, arm);
            float keptFlexion = Math.Clamp(flexion, -range.Extension, range.Flexion), keptUlnar = Math.Clamp(ulnar, -range.Radial, range.Ulnar);
            Quaternion kept = canonical;
            if (keptFlexion != flexion || keptUlnar != ulnar)
            {
                float x = -MathF.Sin(keptUlnar * Radians) * sideways, rest = MathF.Cos(keptUlnar * Radians);
                Vector3 inRange = new(x, rest * MathF.Cos(keptFlexion * Radians), rest * MathF.Sin(keptFlexion * Radians));
                kept = Quaternion.Normalize(canonical * Arc(inRange, Vector3.Normalize(Vector3.Transform(arm.Forearm, Quaternion.Inverse(canonical)))));
            }
            // Then the roll, about the forearm, which leaves the bend as it is.
            float roll = Angles(kept, arm).Roll, keptRoll = Math.Clamp(roll, -range.Roll, range.Roll);
            if (keptRoll != roll) kept = Quaternion.Normalize(Quaternion.CreateFromAxisAngle(arm.Forearm, (keptRoll - roll) * sideways * Radians) * kept);
            float dot = MathF.Min(1, MathF.Abs(Quaternion.Dot(kept, canonical)));
            return (kept, swung != 0 ? a + arm.Bend : pole, new(asked.Flexion, asked.Ulnar, asked.Roll, 2 * MathF.Acos(dot) / Radians, swung));
        }

        internal HandShape Knuckles(JointPose[] joints) =>
            HandKnuckles.Read(joints, Quaternion.Normalize(joints[hand].Model.Rotation * Quaternion.Inverse(Frame)), fingers, thumb, left);

        // Each digit joint's turn over the clip: the rig's grip (to the canonical holding pose), the look's shape, then
        // the closure toward the arms' close at 1 or open at -1. Opening blends the shape toward the open bend, so a
        // shaped finger never bends back past it; squeezing adds to the shape. Curls are negative turns about the curl
        // axis (toward the palm). Each thumb bone is aimed where its clip's [curl, swing] plus the added ones put it in
        // the hand's canonical frame (swing mirrored on the left), so the drawn thumb reads as asked whatever its
        // joints' own frames.
        internal IEnumerable<(uint Joint, Quaternion Turn)> Close(ArmRig rig, ArmDigits digits, HandShape? shape, float closure)
        {
            DigitBend bend = closure > 0 ? digits.Close : digits.Open;
            HandShape grip = left ? rig.Grip.Off ?? rig.Grip.Main : rig.Grip.Main;
            float weight = MathF.Abs(closure), side = left ? -1 : 1;
            float Bend(float shaped, float target) => closure < 0 ? shaped * (1 - weight) + target * weight : shaped + target * weight;
            for (int f = 0; f < fingers.Length; f++)
                for (int i = 0; i < 3; i++)
                {
                    float curl = grip.Fingers[f][i] + Bend(shape?.Fingers[f][i] ?? 0, bend.Fingers[i]);
                    if (curl != 0) yield return (fingers[f][i], Quaternion.CreateFromAxisAngle(curls[fingers[f][i]], -curl * Radians));
                }
            float[][] added = [.. Enumerable.Range(0, 3).Select(i => new[] {
                grip.Thumb[i][0] + Bend(shape?.Thumb[i][0] ?? 0, bend.Thumb[i][0]), grip.Thumb[i][1] + Bend(shape?.Thumb[i][1] ?? 0, bend.Thumb[i][1]) })];
            if (Clip is not { } clip || added.All(a => a[0] == 0 && a[1] == 0)) yield break;
            // A joint's angles are its bone's from the bone before, so each bone's rise and swing in the canonical
            // frame are the sums down the thumb; `moved` is how far the joints above have turned it (canonical frame).
            float rise = 0, swing = 0;
            Quaternion moved = Quaternion.Identity;
            for (int i = 0; i < 3; i++)
            {
                rise += (clip.Thumb[i][0] + added[i][0]) * Radians;
                swing += (clip.Thumb[i][1] + added[i][1]) * Radians * side;
                Vector3 aim = new(-MathF.Cos(rise) * MathF.Sin(swing), MathF.Cos(rise) * MathF.Cos(swing), -MathF.Sin(rise));
                Quaternion placed = Quaternion.Normalize(moved * thumbs[i].Turn);
                Quaternion arc = Arc(Vector3.Transform(thumbs[i].Bone, placed), aim);
                moved = Quaternion.Normalize(arc * moved);
                yield return (thumb[i], Quaternion.Normalize(Quaternion.Inverse(placed) * arc * placed));
            }
        }

        // The shortest turn from one unit direction to another.
        private static Quaternion Arc(Vector3 from, Vector3 to)
        {
            float dot = Vector3.Dot(from, to);
            if (dot < -0.9999f)
            {
                Vector3 across = Vector3.Cross(from, MathF.Abs(from.X) < 0.9f ? Vector3.UnitX : Vector3.UnitY);
                return Quaternion.CreateFromAxisAngle(Vector3.Normalize(across), MathF.PI);
            }
            return Quaternion.Normalize(new Quaternion(Vector3.Cross(from, to), 1 + dot));
        }
    }
}

/// <summary>
/// How a wrist was asked to bend from its forearm on a reach (degrees), with the elbow at its pole: toward the palm
/// (<see cref="Flexion"/>, negative toward the back of the hand), toward the little finger (<see cref="Ulnar"/>, negative
/// toward the thumb) and rolled about the forearm from its neutral turn, the thumb toward the inside of the elbow
/// (<see cref="Roll"/>, positive turning the palm up: supination); how far the hand was turned back to stay in range
/// (<see cref="Clamped"/>, 0 for a hand in range) and how far the elbow swung from its pole toward it
/// (<see cref="Elbow"/>).
/// </summary>
public readonly record struct WristBend(float Flexion, float Ulnar, float Roll, float Clamped, float Elbow);
