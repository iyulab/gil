using System.Runtime.InteropServices;
using System.Text;

namespace Gil.Native;

/// <summary>The C ABI of the native library (crate <c>gil-ffi</c>), one declaration per exported function.</summary>
internal static unsafe partial class Native
{
    private const string Library = "gil_native";

    internal const int Ok = 0;
    internal const int InvalidInput = 1;
    internal const int InvalidArgument = 2;
    internal const int Panic = 3;
    internal const int BufferTooSmall = 4;
    internal const int Broken = 5;

    /// <summary>The version of the C ABI this binding was built for (<c>GIL_ABI_VERSION</c>).</summary>
    internal const uint AbiVersion = 1;

    private static volatile bool _compatible;

    [StructLayout(LayoutKind.Sequential)]
    internal struct Str
    {
        public byte* Ptr;
        public nuint Len;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Candidate
    {
        public uint ValueOffset;
        public uint ValueLen;
        public double Score;
        public byte Source;
        public byte Trusted;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Answer
    {
        public byte Source;
        public byte Policy;
        public byte NeedsExternal;
        public byte Answered;
        public byte HasCoarse;
        public Candidate Coarse;
        public uint Candidates;
        public uint ValueBytes;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct ExternalCandidate
    {
        public Str Value;
        public double Probability;
        public byte Trusted;
    }

    [LibraryImport(Library, EntryPoint = "gil_abi_version")]
    private static partial uint LibraryAbiVersion();

    [LibraryImport(Library, EntryPoint = "gil_last_error")]
    private static partial nuint LastError(byte* buffer, nuint capacity);

    [LibraryImport(Library, EntryPoint = "gil_last_result")]
    private static partial nuint LastResult(byte* buffer, nuint capacity);

    [LibraryImport(Library, EntryPoint = "gil_schema_new")]
    internal static partial int SchemaNew(Str json, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_schema_free")]
    internal static partial void SchemaFree(nint schema);

    [LibraryImport(Library, EntryPoint = "gil_schema_json")]
    internal static partial int SchemaJson(SchemaHandle schema, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_records_new")]
    internal static partial int RecordsNew(Str lines, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_records_len")]
    internal static partial nuint RecordsLen(RecordsHandle records);

    [LibraryImport(Library, EntryPoint = "gil_records_free")]
    internal static partial void RecordsFree(nint records);

    [LibraryImport(Library, EntryPoint = "gil_decider_new")]
    internal static partial int DeciderNew(byte similarRecords, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_decider_free")]
    internal static partial void DeciderFree(nint decider);

    [LibraryImport(Library, EntryPoint = "gil_decider_put")]
    internal static partial int DeciderPut(DeciderHandle decider, SchemaHandle schema, RecordsHandle records);

    [LibraryImport(Library, EntryPoint = "gil_decider_answer")]
    internal static partial int DeciderAnswer(
        DeciderHandle decider, SchemaHandle schema, Str recordId, Str values, Str sets, byte decisions,
        nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_decider_remove")]
    internal static partial int DeciderRemove(DeciderHandle decider, SchemaHandle schema, Str recordId);

    [LibraryImport(Library, EntryPoint = "gil_open_record_new")]
    internal static partial int OpenRecordNew(SchemaHandle schema, Str id, long settledAt, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_open_record_reopen")]
    internal static partial int OpenRecordReopen(SchemaHandle schema, Str record, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_open_record_free")]
    internal static partial void OpenRecordFree(nint record);

    [LibraryImport(Library, EntryPoint = "gil_open_record_observe")]
    internal static partial int OpenRecordObserve(
        OpenRecordHandle record, DeciderHandle decider, Str attribute, Str value, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_open_record_settle")]
    internal static partial int OpenRecordSettle(
        OpenRecordHandle record, DeciderHandle decider, Str attribute, Str kind, long now, Str value, Str* values,
        nuint valuesLen, double probability, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_open_record_answer")]
    internal static partial int OpenRecordAnswer(OpenRecordHandle record, DeciderHandle decider, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_open_record_answer_partial")]
    internal static partial int OpenRecordAnswerPartial(
        OpenRecordHandle record, DeciderHandle decider, Str attribute, Str partial, Answer* answer,
        Candidate* candidates, nuint candidatesCapacity, byte* valueBytes, nuint valueBytesCapacity);

    [LibraryImport(Library, EntryPoint = "gil_open_record_offer_external")]
    internal static partial int OpenRecordOfferExternal(
        OpenRecordHandle record, DeciderHandle decider, Str attribute, ExternalCandidate* ranked, nuint rankedLen,
        nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_open_record_snapshot")]
    internal static partial int OpenRecordSnapshot(OpenRecordHandle record, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_select_layers")]
    internal static partial int SelectLayers(
        SchemaHandle schema, Str attribute, RecordsHandle records, Str options, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_select_coarse")]
    internal static partial int SelectCoarse(
        SchemaHandle schema, Str attribute, RecordsHandle records, Str options, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_select_key_fields")]
    internal static partial int SelectKeyFields(
        SchemaHandle schema, Str attribute, RecordsHandle records, Str options, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_select_key_concentrations")]
    internal static partial int SelectKeyConcentrations(
        SchemaHandle schema, Str attribute, RecordsHandle records, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_tuning_capture")]
    internal static partial int TuningCapture(SchemaHandle schema, nuint settled, double targetPrecision, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_tuning_promise")]
    internal static partial int TuningPromise(Str tuning, Str attribute, Str thresholds, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_tuning_apply")]
    internal static partial int TuningApply(Str tuning, SchemaHandle schema, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_tuning_stale")]
    internal static partial int TuningStale(Str tuning, SchemaHandle schema, nuint settled, nuint* resultLen);

    [StructLayout(LayoutKind.Sequential)]
    internal struct Decision
    {
        public Str Attribute;
        public Str Answer;
        public Str Settled;
        public Str Settlement;
        public double Probability;
        public double Promise;
    }

    [LibraryImport(Library, EntryPoint = "gil_usage")]
    internal static partial int Usage(Decision* decisions, nuint decisionsLen, nuint* resultLen);

    /// <summary>Makes sure the native library speaks the interface this binding was built for — once, before the first
    /// object is made.</summary>
    /// <exception cref="InvalidOperationException">The library speaks another version.</exception>
    internal static void EnsureCompatible()
    {
        if (_compatible)
        {
            return;
        }

        var found = LibraryAbiVersion();
        if (found != AbiVersion)
        {
            throw new InvalidOperationException(
                $"The native library {Library} speaks version {found} of its interface; this binding needs version " +
                $"{AbiVersion}. Use the native library built with this binding.");
        }

        _compatible = true;
    }

    /// <summary>Throws for a refused call, with the native message: a refused input, or a call that only reads and that
    /// the core's contract rules out, is the caller's <see cref="ArgumentException"/>; a malformed call is a defect of
    /// this binding.</summary>
    internal static void Check(int status) => Check(status, changing: false);

    /// <summary>As <see cref="Check(int)"/>, for a call that changes a decider (and a record): a refused contract comes
    /// back as input refused before anything changed, so a panic here is a defect that may have stopped the change
    /// halfway — the core marks what it was changing broken, and so does the exception.</summary>
    internal static void Check(int status, bool changing)
    {
        if (status == Ok)
        {
            return;
        }

        var message = Copy(&LastError);
        throw status switch
        {
            Panic when changing => new InvalidOperationException(
                $"The Gil core failed while changing the decider; it and the record can no longer be used: {message}"),
            InvalidInput or Panic => new ArgumentException(message),
            Broken => new InvalidOperationException(message),
            _ => new InvalidOperationException($"The Gil core refused a call from its binding ({status}): {message}"),
        };
    }

    /// <summary>The text result of the call just made, after <see cref="Check(int, bool)"/> passed it.</summary>
    internal static string Result(int status, bool changing = false)
    {
        Check(status, changing);
        return Copy(&LastResult);
    }

    private static string Copy(delegate*<byte*, nuint, nuint> copy)
    {
        var length = copy(null, 0);
        if (length == 0)
        {
            return string.Empty;
        }

        Span<byte> buffer = length <= 1024 ? stackalloc byte[(int)length] : new byte[(int)length];
        fixed (byte* p = buffer)
        {
            copy(p, length);
        }

        return Encoding.UTF8.GetString(buffer);
    }
}

/// <summary>A string encoded to UTF-8 for one call, pinned while the call runs; a null string is null text.</summary>
internal readonly ref struct Utf8
{
    private readonly byte[]? _bytes;

    public Utf8(string? text) => _bytes = text is null ? null : Encoding.UTF8.GetBytes(text);

    /// <summary>The text for the native call, given the address <see cref="Pin"/> gave.</summary>
    public unsafe Native.Str At(byte* pinned) => _bytes is null
        ? default
        : new() { Ptr = pinned, Len = (nuint)_bytes.Length };

    public ref readonly byte Pin() => ref _bytes is { Length: > 0 } ? ref _bytes[0] : ref Empty[0];

    private static readonly byte[] Empty = [0];
}
