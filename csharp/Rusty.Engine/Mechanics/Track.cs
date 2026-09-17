namespace Rusty.Engine.Mechanics;

public enum TrackMaximumChangePolicy { PreserveCurrent, PreserveMissingAmount }

/// <summary>Current resource value with a shared Stat maximum. Ordinary changes are direct methods.</summary>
public sealed class Track
{
    private double _current;
    private double _minimum;

    public Track(Stat maximum, double? current = null, double minimum = 0,
        TrackMaximumChangePolicy maximumChangePolicy = TrackMaximumChangePolicy.PreserveCurrent,
        double quantum = 0, MidpointRounding rounding = MidpointRounding.AwayFromZero,
        MidpointRounding integerRounding = MidpointRounding.AwayFromZero)
    {
        Maximum = maximum ?? throw new ArgumentNullException(nameof(maximum));
        Stat.Finite(minimum, nameof(minimum));
        Stat.Finite(quantum, nameof(quantum));
        if (minimum > maximum.Value) throw new ArgumentException("Track minimum exceeds its maximum.");
        if (quantum < 0) throw new ArgumentOutOfRangeException(nameof(quantum));
        if (!Enum.IsDefined(maximumChangePolicy)) throw new ArgumentOutOfRangeException(nameof(maximumChangePolicy));
        if (!Enum.IsDefined(rounding)) throw new ArgumentOutOfRangeException(nameof(rounding));
        if (!Enum.IsDefined(integerRounding)) throw new ArgumentOutOfRangeException(nameof(integerRounding));
        _minimum = minimum;
        MaximumChangePolicy = maximumChangePolicy;
        Quantum = quantum;
        Rounding = rounding;
        IntegerRounding = integerRounding;
        SetCurrent(current ?? maximum.Value);
        maximum.Attach(this);
    }

    public Track(double maximum, double? current = null, double minimum = 0,
        TrackMaximumChangePolicy maximumChangePolicy = TrackMaximumChangePolicy.PreserveCurrent,
        double quantum = 0, MidpointRounding rounding = MidpointRounding.AwayFromZero,
        MidpointRounding integerRounding = MidpointRounding.AwayFromZero)
        : this(new Stat(maximum), current, minimum, maximumChangePolicy, quantum, rounding, integerRounding) { }

    public Stat Maximum { get; }
    public double MaximumValue => Maximum.Value;
    public double Minimum
    {
        get => _minimum;
        set
        {
            Stat.Finite(value, nameof(value));
            if (value > MaximumValue) throw new ArgumentException("Track minimum exceeds its maximum.");
            double next = Math.Clamp(_current, value, MaximumValue);
            _minimum = value;
            _current = next;
        }
    }
    public double Current { get => _current; set => SetCurrent(value); }
    public double Value => _current;
    public float ValueFloat => Stat.ToFloat(_current);
    public int ValueInt => checked((int)Math.Round(_current, IntegerRounding));
    public long ValueInt64 => checked((long)Math.Round(_current, IntegerRounding));
    public double Quantum { get; }
    public MidpointRounding Rounding { get; }
    public MidpointRounding IntegerRounding { get; }
    public TrackMaximumChangePolicy MaximumChangePolicy { get; }

    public void SetCurrent(double value, bool clamp = false)
    {
        Stat.Finite(value, nameof(value));
        if (!clamp && (value < Minimum || value > MaximumValue))
            throw new ArgumentOutOfRangeException(nameof(value), "Current value is outside track bounds.");
        _current = Resolve(value, Minimum, MaximumValue);
    }

    public bool TrySpend(double amount)
    {
        ValidateAmount(amount);
        if (amount == 0) return true;
        if (amount > _current - Minimum) return false;
        double next = Math.Min(_current, Resolve(_current - amount, Minimum, MaximumValue));
        AppliedAmount(_current - next);
        _current = next;
        return true;
    }

    public double Spend(double amount)
    {
        double before = _current;
        if (!TrySpend(amount)) throw new MechanicsException("Insufficient track value.");
        return before - _current;
    }

    public double Restore(double amount)
    {
        ValidateAmount(amount);
        if (amount == 0) return 0;
        double before = _current;
        // A positive overflow still has a well-defined saturating result.
        double next = Math.Max(_current, Resolve(Math.Min(MaximumValue, _current + amount), Minimum, MaximumValue));
        double applied = AppliedAmount(next - before);
        _current = next;
        return applied;
    }

    private static double AppliedAmount(double amount)
    {
        if (!double.IsFinite(amount)) throw new OverflowException("Applied track amount does not fit Double.");
        return amount;
    }

    private static void ValidateAmount(double amount)
    {
        Stat.Finite(amount, nameof(amount));
        if (amount < 0) throw new ArgumentOutOfRangeException(nameof(amount));
    }

    private double Resolve(double current, double minimum, double maximum)
    {
        // Endpoints stay reachable even when they are off the quantization grid.
        if (current <= minimum) return minimum;
        if (current >= maximum) return maximum;
        return Math.Clamp(Stat.Quantize(current, Quantum, Rounding), minimum, maximum);
    }

    // Only Stat calls these methods. It validates all dependents before publishing any of them.
    internal double PrepareMaximum(double maximum)
    {
        if (maximum < Minimum) throw new MechanicsException("Stat change would put a track maximum below its minimum.");
        if (maximum == MaximumValue) return _current;
        if (MaximumChangePolicy == TrackMaximumChangePolicy.PreserveCurrent)
            return Math.Clamp(_current, Minimum, maximum);
        double next = _current;
        if (MaximumChangePolicy == TrackMaximumChangePolicy.PreserveMissingAmount)
        {
            double change = maximum - MaximumValue;
            next = double.IsFinite(change)
                ? _current + change
                : maximum - (MaximumValue - _current);
        }
        return Resolve(next, Minimum, maximum);
    }

    internal void ApplyMaximum(double current) => _current = current;
}
