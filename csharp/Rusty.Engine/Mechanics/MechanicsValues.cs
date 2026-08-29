using System.Numerics;

namespace Rusty.Engine.Mechanics;

/// <summary>
/// A bounded signed integer used by exact managed mechanics.
/// </summary>
public readonly record struct ExactValue : IComparable<ExactValue>
{
    public const long MaximumAbsolute = 1_000_000_000_000;

    public ExactValue(long raw)
    {
        if (raw < -MaximumAbsolute || raw > MaximumAbsolute)
        {
            throw new ArgumentOutOfRangeException(
                nameof(raw), raw, $"Exact values must be within ±{MaximumAbsolute}.");
        }

        Raw = raw;
    }

    public long Raw { get; }

    public static ExactValue Zero => new(0);

    public ExactValue CheckedAdd(ExactValue other)
    {
        long value;
        try
        {
            value = checked(Raw + other.Raw);
        }
        catch (OverflowException exception)
        {
            throw new MechanicsArithmeticException("Exact addition overflowed.", exception);
        }

        return new ExactValue(value);
    }

    public ExactValue CheckedSubtract(ExactValue other)
    {
        long value;
        try
        {
            value = checked(Raw - other.Raw);
        }
        catch (OverflowException exception)
        {
            throw new MechanicsArithmeticException("Exact subtraction overflowed.", exception);
        }

        return new ExactValue(value);
    }

    public ExactValue RequireNonNegative()
    {
        if (Raw < 0)
        {
            throw new MechanicsArithmeticException($"Exact amount {Raw} cannot be negative.");
        }

        return this;
    }

    public ExactValue Clamp(ExactValue minimum, ExactValue maximum)
    {
        if (minimum > maximum)
        {
            throw new MechanicsArithmeticException("Exact clamp bounds are inverted.");
        }

        return new ExactValue(Math.Clamp(Raw, minimum.Raw, maximum.Raw));
    }

    public int CompareTo(ExactValue other) => Raw.CompareTo(other.Raw);

    public static bool operator <(ExactValue left, ExactValue right) => left.Raw < right.Raw;

    public static bool operator >(ExactValue left, ExactValue right) => left.Raw > right.Raw;

    public static bool operator <=(ExactValue left, ExactValue right) => left.Raw <= right.Raw;

    public static bool operator >=(ExactValue left, ExactValue right) => left.Raw >= right.Raw;

    public override string ToString() => Raw.ToString(System.Globalization.CultureInfo.InvariantCulture);
}

/// <summary>
/// A normalized positive rational used for checked exact scaling. Scaling is
/// applied with one final toward-zero rounding step.
/// </summary>
public readonly record struct ExactRatio : IComparable<ExactRatio>
{
    public const uint MaximumComponent = 1_000_000;

    public ExactRatio(uint numerator, uint denominator)
    {
        if (denominator == 0)
        {
            throw new MechanicsArithmeticException("Exact ratios cannot have a zero denominator.");
        }

        if (numerator > MaximumComponent || denominator > MaximumComponent)
        {
            throw new ArgumentOutOfRangeException(
                nameof(numerator),
                $"Exact ratio components must be at most {MaximumComponent}.");
        }

        uint divisor = GreatestCommonDivisor(numerator, denominator);
        Numerator = numerator / divisor;
        Denominator = denominator / divisor;
    }

    public uint Numerator { get; }

    public uint Denominator { get; }

    public ExactValue Apply(ExactValue value)
    {
        EnsureValid();
        return ExactRatioProduct.One.Include(this).Apply(value);
    }

    public int CompareTo(ExactRatio other)
    {
        EnsureValid();
        other.EnsureValid();
        ulong left = (ulong)Numerator * other.Denominator;
        ulong right = (ulong)other.Numerator * Denominator;
        return left.CompareTo(right);
    }

    internal void EnsureValid()
    {
        if (Denominator == 0)
        {
            throw new MechanicsArithmeticException("Exact ratios cannot have a zero denominator.");
        }

        if (Numerator > MaximumComponent || Denominator > MaximumComponent)
        {
            throw new MechanicsArithmeticException(
                $"Exact ratio components must be at most {MaximumComponent}.");
        }
    }

    private static uint GreatestCommonDivisor(uint left, uint right)
    {
        while (right != 0)
        {
            uint remainder = left % right;
            left = right;
            right = remainder;
        }

        return left;
    }
}

/// <summary>
/// An immutable product of exact ratios. BigInteger keeps intermediate ratio
/// products checked without rounding or silently overflowing before the final
/// bounded <see cref="ExactValue"/> result.
/// </summary>
public sealed class ExactRatioProduct
{
    private ExactRatioProduct(BigInteger numerator, BigInteger denominator)
    {
        Numerator = numerator;
        Denominator = denominator;
    }

    public BigInteger Numerator { get; }

    public BigInteger Denominator { get; }

    public static ExactRatioProduct One => new(BigInteger.One, BigInteger.One);

    public ExactRatioProduct Include(ExactRatio ratio)
    {
        ratio.EnsureValid();
        BigInteger numerator = Numerator * ratio.Numerator;
        BigInteger denominator = Denominator * ratio.Denominator;
        BigInteger divisor = BigInteger.GreatestCommonDivisor(numerator, denominator);
        return new(numerator / divisor, denominator / divisor);
    }

    public ExactValue Apply(ExactValue value)
    {
        if (Denominator <= 0 || Numerator < 0)
        {
            throw new MechanicsArithmeticException("Exact ratio products must be non-negative with a positive denominator.");
        }

        BigInteger magnitude = BigInteger.Abs(new BigInteger(value.Raw)) * Numerator / Denominator;
        BigInteger signed = value.Raw < 0 ? -magnitude : magnitude;
        if (signed < -ExactValue.MaximumAbsolute || signed > ExactValue.MaximumAbsolute)
        {
            throw new MechanicsArithmeticException("Exact scaling exceeded the admitted value range.");
        }

        return new ExactValue((long)signed);
    }
}

/// <summary>Rounding policy for exact arithmetic.</summary>
public enum ExactRoundingPolicy
{
    TowardZero,
}

/// <summary>
/// A finite IEEE-754 binary64 value. Negative zero is normalized to positive
/// zero, making equality and persisted bit identity deterministic.
/// </summary>
public readonly record struct ContinuousValue : IComparable<ContinuousValue>
{
    public ContinuousValue(double value)
    {
        if (!double.IsFinite(value))
        {
            throw new MechanicsArithmeticException("Continuous values must be finite.");
        }

        Value = value == 0.0 ? 0.0 : value;
    }

    public double Value { get; }

    public ulong Bits => BitConverter.DoubleToUInt64Bits(Value);

    public static ContinuousValue Zero => new(0.0);

    public static ContinuousValue FromBits(ulong bits) =>
        new(BitConverter.UInt64BitsToDouble(bits));

    public ContinuousValue CheckedAdd(ContinuousValue other) =>
        Checked(Value + other.Value, "Continuous addition became non-finite.");

    public ContinuousValue CheckedSubtract(ContinuousValue other) =>
        Checked(Value - other.Value, "Continuous subtraction became non-finite.");

    public ContinuousValue CheckedMultiply(ContinuousValue other) =>
        Checked(Value * other.Value, "Continuous multiplication became non-finite.");

    public ContinuousValue CheckedDivide(ContinuousValue other)
    {
        if (other.Value == 0.0)
        {
            throw new MechanicsArithmeticException("Continuous division cannot use zero.");
        }

        return Checked(Value / other.Value, "Continuous division became non-finite.");
    }

    public ContinuousValue Clamp(ContinuousValue minimum, ContinuousValue maximum)
    {
        if (minimum > maximum)
        {
            throw new MechanicsArithmeticException("Continuous clamp bounds are inverted.");
        }

        return new ContinuousValue(Math.Clamp(Value, minimum.Value, maximum.Value));
    }

    public ContinuousValue RequireNonNegative()
    {
        if (Value < 0.0)
        {
            throw new MechanicsArithmeticException($"Continuous amount {Value} cannot be negative.");
        }

        return this;
    }

    public int CompareTo(ContinuousValue other) => Value.CompareTo(other.Value);

    public static bool operator <(ContinuousValue left, ContinuousValue right) => left.Value < right.Value;

    public static bool operator >(ContinuousValue left, ContinuousValue right) => left.Value > right.Value;

    public static bool operator <=(ContinuousValue left, ContinuousValue right) => left.Value <= right.Value;

    public static bool operator >=(ContinuousValue left, ContinuousValue right) => left.Value >= right.Value;

    public override string ToString() => Value.ToString(System.Globalization.CultureInfo.InvariantCulture);

    private static ContinuousValue Checked(double value, string message)
    {
        return double.IsFinite(value)
            ? new ContinuousValue(value)
            : throw new MechanicsArithmeticException(message);
    }
}

/// <summary>Exception raised by a managed mechanics numeric invariant.</summary>
public sealed class MechanicsArithmeticException : InvalidOperationException
{
    public MechanicsArithmeticException(string message) : base(message) { }

    public MechanicsArithmeticException(string message, Exception innerException)
        : base(message, innerException) { }
}
