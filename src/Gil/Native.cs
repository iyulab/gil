using System.Runtime.InteropServices;

namespace Gil;

/// <summary>The C ABI of the native library (crate <c>gil-ffi</c>), one declaration per exported function; what every
/// binding of it shares is in NativeInterop.cs.</summary>
internal static unsafe partial class Native
{
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
    internal static partial int DeciderNew(byte similarRecords, nuint threads, nint* output);

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
    internal static partial int OpenRecordAnswer(
        OpenRecordHandle record, DeciderHandle decider, byte decisions, nuint* resultLen);

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

    [LibraryImport(Library, EntryPoint = "gil_tuning_promise_coarse")]
    internal static partial int TuningPromiseCoarse(Str tuning, Str attribute, Str thresholds, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_schema_with_layers")]
    internal static partial int SchemaWithLayers(SchemaHandle schema, Str attribute, Str thresholds, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_schema_with_coarse")]
    internal static partial int SchemaWithCoarse(SchemaHandle schema, Str attribute, Str thresholds, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_tuning_apply")]
    internal static partial int TuningApply(Str tuning, SchemaHandle schema, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_tuning_stale")]
    internal static partial int TuningStale(Str tuning, SchemaHandle schema, nuint settled, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_keeper_new")]
    internal static partial int KeeperNew(SchemaHandle schema, Str options, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_keeper_free")]
    internal static partial void KeeperFree(nint keeper);

    [LibraryImport(Library, EntryPoint = "gil_keeper_settle")]
    internal static partial int KeeperSettle(KeeperHandle keeper, DeciderHandle decider, RecordsHandle records);

    [LibraryImport(Library, EntryPoint = "gil_keeper_remove")]
    internal static partial int KeeperRemove(KeeperHandle keeper, DeciderHandle decider, Str recordId);

    [LibraryImport(Library, EntryPoint = "gil_keeper_stale")]
    internal static partial int KeeperStale(KeeperHandle keeper, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_keeper_settled")]
    internal static partial nuint KeeperSettled(KeeperHandle keeper);

    [LibraryImport(Library, EntryPoint = "gil_keeper_rechoose")]
    internal static partial int KeeperRechoose(KeeperHandle keeper, byte decisions, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_keeper_state")]
    internal static partial int KeeperState(KeeperHandle keeper, nuint* resultLen);

    [LibraryImport(Library, EntryPoint = "gil_keeper_schema")]
    internal static partial int KeeperSchema(KeeperHandle keeper, nint* output);

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

    [LibraryImport(Library, EntryPoint = "gil_ask_planner_new")]
    internal static partial int AskPlannerNew(
        SchemaHandle schema, Str attribute, Str* askable, nuint askableLen, double budget, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_ask_planner_free")]
    internal static partial void AskPlannerFree(nint planner);

    [LibraryImport(Library, EntryPoint = "gil_ask_planner_learn")]
    internal static partial int AskPlannerLearn(AskPlannerHandle planner, DeciderHandle decider, RecordsHandle records);

    [LibraryImport(Library, EntryPoint = "gil_ask_planner_advise")]
    internal static partial int AskPlannerAdvise(AskPlannerHandle planner, Str answer, Str values, nuint* resultLen);
}
