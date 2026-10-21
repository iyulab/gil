using System.Runtime.InteropServices;

namespace Gil;

/// <summary>The lexical memory's part of the C ABI.</summary>
internal static unsafe partial class Native
{
    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_new")]
    internal static partial int LexicalMemoryNew(nuint minGram, nuint maxGram, nint* output);

    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_free")]
    internal static partial void LexicalMemoryFree(nint memory);

    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_count")]
    internal static partial int LexicalMemoryCount(LexicalMemoryHandle memory, Str task, nuint* count);

    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_remember")]
    internal static partial int LexicalMemoryRemember(LexicalMemoryHandle memory, Str task, Str key, Str state, Str answer);

    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_forget")]
    internal static partial int LexicalMemoryForget(LexicalMemoryHandle memory, Str task, Str key);

    [LibraryImport(Library, EntryPoint = "gil_lexical_memory_nearest")]
    internal static partial int LexicalMemoryNearest(
        LexicalMemoryHandle memory, Str task, Str state, nuint count, nuint* resultLen);
}

internal sealed class LexicalMemoryHandle : NativeHandle
{
    public static LexicalMemoryHandle Of(nint value) => Own<LexicalMemoryHandle>(value);

    protected override bool ReleaseHandle()
    {
        Native.LexicalMemoryFree(handle);
        return true;
    }
}
