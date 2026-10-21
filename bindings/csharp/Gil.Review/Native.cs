using System.Runtime.InteropServices;
using System.Text;

namespace Gil.Review;

/// <summary>The C ABI of the native library (crate <c>gil-review-ffi</c>), one declaration per exported function.</summary>
internal static unsafe partial class Native
{
    private const string Library = "gil_review_native";

    internal const int Ok = 0;
    internal const int InvalidInput = 1;
    internal const int InvalidArgument = 2;
    internal const int Panic = 3;

    [StructLayout(LayoutKind.Sequential)]
    internal struct Str
    {
        public byte* Ptr;
        public nuint Len;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct PopulationStratum
    {
        public Str Name;
        public nuint First;
        public nuint Count;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct DrawnStratum
    {
        public ulong Population;
        public ulong Selected;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct StratumSpread
    {
        public ulong Population;
        public double Deviation;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Stratum
    {
        public Str Name;
        public ulong Population;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct ScoredItem
    {
        public uint Stratum;
        public double Score;
        public double Weight;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Estimate
    {
        public double Point;
        public double Se;
        public double NormalLow;
        public double NormalHigh;
        public double BootstrapLow;
        public double BootstrapHigh;
        public ulong SampleSize;
        public ulong Population;
        public double EffectiveN;
        public ulong Seed;
        public uint Caveats;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Verdict
    {
        public Str Item;
        public Str Candidate;
        public Str Reviewer;
        public byte Outcome;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Agreement
    {
        public ulong ComparedPairs;
        public double Observed;
        public double Kappa;
        public double Ac1;
        public double PrevalenceMax;
    }

    [LibraryImport(Library, EntryPoint = "gil_review_last_error")]
    private static partial nuint LastError(byte* buffer, nuint capacity);

    [LibraryImport(Library, EntryPoint = "gil_review_draw_pilot")]
    internal static partial int DrawPilot(
        PopulationStratum* strata, nuint strataLen, Str* recordIds, nuint recordIdsLen, uint perStratum, ulong seed,
        DrawnStratum* drawn, uint* selected, nuint selectedCapacity, uint* caveats);

    [LibraryImport(Library, EntryPoint = "gil_review_allocate_neyman")]
    internal static partial int AllocateNeyman(StratumSpread* strata, nuint strataLen, ulong budget, ulong* counts);

    [LibraryImport(Library, EntryPoint = "gil_review_minimum_design_deviation")]
    internal static partial double MinimumDesignDeviation();

    [LibraryImport(Library, EntryPoint = "gil_review_unobserved_variation_bound")]
    internal static partial double UnobservedVariationBound(ulong reviewed);

    [LibraryImport(Library, EntryPoint = "gil_review_estimate")]
    internal static partial int Estimate_(
        Stratum* strata, nuint strataLen, ScoredItem* items, nuint itemsLen, uint replicates, ulong seed,
        Estimate* output);

    [LibraryImport(Library, EntryPoint = "gil_review_estimate_bounds")]
    internal static partial int EstimateBounds(
        Stratum* strata, nuint strataLen, ScoredItem* lower, nuint lowerLen, ScoredItem* upper, nuint upperLen,
        uint replicates, ulong seed, Estimate* low, Estimate* high);

    [LibraryImport(Library, EntryPoint = "gil_review_agreement")]
    internal static partial int Agreement_(
        Verdict* verdicts, nuint verdictsLen, byte categories, Agreement* output, double* specific);

    /// <summary>Throws for a refused call, with the native message: a refused input is the caller's
    /// <see cref="ArgumentException"/>; anything else is a defect of the binding or the library.</summary>
    internal static void Check(int status)
    {
        if (status == Ok)
        {
            return;
        }

        var message = ReadLastError();
        throw status == InvalidInput
            ? new ArgumentException(message)
            : new InvalidOperationException($"The review library refused a call from its binding ({status}): {message}");
    }

    private static string ReadLastError()
    {
        var length = LastError(null, 0);
        if (length == 0)
        {
            return string.Empty;
        }

        var buffer = new byte[(int)length];
        fixed (byte* p = buffer)
        {
            LastError(p, length);
        }

        return Encoding.UTF8.GetString(buffer);
    }
}

/// <summary>Many strings encoded once into one UTF-8 block, so one pin covers them all for a call.</summary>
internal readonly struct Utf8Block
{
    private readonly int[] _offsets;

    private Utf8Block(byte[] bytes, int[] offsets)
    {
        Bytes = bytes;
        _offsets = offsets;
    }

    public byte[] Bytes { get; }

    public static Utf8Block Of(IReadOnlyList<string> strings)
    {
        var offsets = new int[strings.Count + 1];
        var total = 0;
        for (var i = 0; i < strings.Count; i++)
        {
            offsets[i] = total;
            total += Encoding.UTF8.GetByteCount(strings[i]);
        }

        offsets[strings.Count] = total;
        var bytes = new byte[total];
        for (var i = 0; i < strings.Count; i++)
        {
            Encoding.UTF8.GetBytes(strings[i], bytes.AsSpan(offsets[i]));
        }

        return new Utf8Block(bytes, offsets);
    }

    /// <summary>The <paramref name="index"/>th string, given the pinned address of <see cref="Bytes"/>.</summary>
    public unsafe Native.Str At(byte* pinned, int index) => new()
    {
        Ptr = pinned + _offsets[index],
        Len = (nuint)(_offsets[index + 1] - _offsets[index]),
    };
}
