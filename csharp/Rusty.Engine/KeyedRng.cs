using System;
using System.Text;
using System.Text.Unicode;

namespace Rusty.Engine;

/// <summary>
/// The Engine's keyed draw (svc-rng's <c>KeyedRngV1</c>) computed in managed code: the same value
/// <see cref="IRandomService.DrawKeyed"/> returns for the same seed, scope, key and range. A draw
/// depends only on its arguments, holds no state and allocates nothing, so it may be called from
/// any thread, a preparation job or a sampler included. Scoped streams and every other Engine
/// service stay on the update.
/// </summary>
/// <remarks>
/// The scope and key are hashed as their UTF-8 bytes, as <c>DrawKeyed</c> encodes its strings, so
/// a byte key holding a string's UTF-8 bytes draws what the string draws. Any other encoding of
/// the same decision (raw little-endian integers, say) draws different values: switching an
/// existing decision to it changes what that decision drew before, so keep it to new decisions.
/// </remarks>
public static class KeyedRng
{
    private const ulong FnvOffset = 0xcbf2_9ce4_8422_2325;
    private const ulong FnvPrime = 0x0000_0100_0000_01b3;
    // KEYED_RNG_V1_MAX_ATTEMPTS: an attempt is rejected less than half the time.
    private const uint MaxAttempts = 128;
    private const int EncodeChunkBytes = 256;

    private static ReadOnlySpan<byte> DomainTag => "rusty-engine/svc-rng/keyed-i64/v1"u8;
    // Every draw's input frame starts with the same tagged bytes: hashed once.
    private static readonly ulong InputPrefix = FeedTagged(FnvOffset, DomainTag, "input"u8);

    /// <summary>
    /// An unbiased value from <paramref name="minimum"/> to <paramref name="maximum"/> inclusive,
    /// fixed by <paramref name="seed"/>, <paramref name="scope"/> (the decision domain) and
    /// <paramref name="key"/> (the decision in it), each string taken as its UTF-8 bytes.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="minimum"/> is greater than <paramref name="maximum"/>.</exception>
    public static long Draw(ulong seed, ReadOnlySpan<char> scope, ReadOnlySpan<char> key, long minimum, long maximum)
    {
        RequireRange(minimum, maximum);
        ulong hash = Input(seed);
        hash = FeedTaggedUtf8(hash, "scope"u8, scope);
        hash = FeedTaggedUtf8(hash, "key"u8, key);
        return Map(hash, minimum, maximum);
    }

    /// <summary>
    /// <see cref="Draw(ulong, ReadOnlySpan{char}, ReadOnlySpan{char}, long, long)"/> with the scope's
    /// and key's bytes given (a <c>"scope"u8</c> literal, a key packed into a stack buffer), for a
    /// hot loop that keys without building strings.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="minimum"/> is greater than <paramref name="maximum"/>.</exception>
    public static long Draw(ulong seed, ReadOnlySpan<byte> scope, ReadOnlySpan<byte> key, long minimum, long maximum)
    {
        RequireRange(minimum, maximum);
        ulong hash = Input(seed);
        hash = FeedTagged(hash, "scope"u8, scope);
        hash = FeedTagged(hash, "key"u8, key);
        return Map(hash, minimum, maximum);
    }

    private static void RequireRange(long minimum, long maximum)
    {
        if (minimum > maximum)
            throw new ArgumentOutOfRangeException(nameof(minimum), minimum, $"a keyed draw's minimum is above its maximum ({maximum})");
    }

    private static ulong Input(ulong seed) => FeedU64(InputPrefix, seed);

    private static long Map(ulong inputHash, long minimum, long maximum)
    {
        ulong span = unchecked((ulong)maximum - (ulong)minimum + 1);
        if (span == 0) return unchecked(minimum + (long)Sample(inputHash, 0));
        // Multiply-high rejection: drop the 2^64 mod span samples that would favour low offsets.
        ulong threshold = unchecked(0 - span) % span;
        for (uint attempt = 0; attempt < MaxAttempts; attempt++)
        {
            ulong offset = Math.BigMul(Sample(inputHash, attempt), span, out ulong low);
            if (low >= threshold) return unchecked(minimum + (long)offset);
        }
        throw new InvalidOperationException($"A keyed draw rejected {MaxAttempts} samples in a row.");
    }

    private static ulong Sample(ulong inputHash, uint attempt) =>
        SplitMix64(FeedU64(FeedTagged(inputHash, DomainTag, "attempt"u8), attempt));

    private static ulong FeedTagged(ulong hash, ReadOnlySpan<byte> tag, ReadOnlySpan<byte> value)
    {
        hash = FeedBytes(FeedU64(hash, (ulong)tag.Length), tag);
        return FeedBytes(FeedU64(hash, (ulong)value.Length), value);
    }

    private static ulong FeedTaggedUtf8(ulong hash, ReadOnlySpan<byte> tag, ReadOnlySpan<char> value)
    {
        hash = FeedBytes(FeedU64(hash, (ulong)tag.Length), tag);
        hash = FeedU64(hash, (ulong)Encoding.UTF8.GetByteCount(value));
        // Encoded a chunk at a time; an unpaired surrogate becomes U+FFFD, as Encoding.UTF8 writes it.
        Span<byte> chunk = stackalloc byte[EncodeChunkBytes];
        while (!value.IsEmpty)
        {
            Utf8.FromUtf16(value, chunk, out int read, out int written);
            hash = FeedBytes(hash, chunk[..written]);
            value = value[read..];
        }
        return hash;
    }

    private static ulong FeedBytes(ulong hash, ReadOnlySpan<byte> bytes)
    {
        foreach (byte value in bytes) hash = unchecked((hash ^ value) * FnvPrime);
        return hash;
    }

    private static ulong FeedU64(ulong hash, ulong value)
    {
        for (int shift = 0; shift < 64; shift += 8) hash = unchecked((hash ^ ((value >> shift) & 0xff)) * FnvPrime);
        return hash;
    }

    private static ulong SplitMix64(ulong x)
    {
        unchecked
        {
            ulong z = x + 0x9e37_79b9_7f4a_7c15;
            z = (z ^ (z >> 30)) * 0xbf58_476d_1ce4_e5b9;
            z = (z ^ (z >> 27)) * 0x94d0_49bb_1331_11eb;
            return z ^ (z >> 31);
        }
    }
}
