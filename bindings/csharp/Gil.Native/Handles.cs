using System.Runtime.InteropServices;

namespace Gil.Native;

/// <summary>A native object owned through a handle the native library handed out, freed once when released.</summary>
internal abstract class NativeHandle : SafeHandle
{
    protected NativeHandle()
        : base(IntPtr.Zero, ownsHandle: true)
    {
    }

    public override bool IsInvalid => handle == IntPtr.Zero;

    /// <summary>Takes ownership of a handle the native library just handed out.</summary>
    protected static T Own<T>(nint value)
        where T : NativeHandle, new()
    {
        var owned = new T();
        owned.SetHandle(value);
        return owned;
    }
}

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

internal sealed class OpenRecordHandle : NativeHandle
{
    public static OpenRecordHandle Of(nint value) => Own<OpenRecordHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.OpenRecordFree(handle);
        return true;
    }
}
