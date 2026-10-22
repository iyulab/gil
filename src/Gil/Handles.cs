namespace Gil;

internal sealed class SchemaHandle : NativeHandle
{
    public static SchemaHandle Of(nint value) => Own<SchemaHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.SchemaFree(handle);
        return true;
    }
}

internal sealed class RecordsHandle : NativeHandle
{
    public static RecordsHandle Of(nint value) => Own<RecordsHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.RecordsFree(handle);
        return true;
    }
}

internal sealed class DeciderHandle : NativeHandle
{
    public static DeciderHandle Of(nint value) => Own<DeciderHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.DeciderFree(handle);
        return true;
    }
}

internal sealed class KeeperHandle : NativeHandle
{
    public static KeeperHandle Of(nint value) => Own<KeeperHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.KeeperFree(handle);
        return true;
    }
}

internal sealed class OpenRecordHandle : NativeHandle
{
    public static OpenRecordHandle Of(nint value) => Own<OpenRecordHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.OpenRecordFree(handle);
        return true;
    }
}

internal sealed class AskPlannerHandle : NativeHandle
{
    public static AskPlannerHandle Of(nint value) => Own<AskPlannerHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.AskPlannerFree(handle);
        return true;
    }
}
