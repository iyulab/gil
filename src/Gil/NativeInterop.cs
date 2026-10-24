using System.Runtime.InteropServices;
using System.Text;

namespace Gil;

// What every .NET binding of the native library (crate gil-ffi) shares: the statuses, text, the version check, errors
// and results, owned handles. Compiled into each assembly that calls the library (Gil, and Gil.Tree as a linked file),
// so that none needs another's internals; each keeps its own declarations in its own part of this class.
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
    internal const uint AbiVersion = 7;

    private static volatile bool _compatible;

    [StructLayout(LayoutKind.Sequential)]
    internal struct Str
    {
        public byte* Ptr;
        public nuint Len;
    }

    [LibraryImport(Library, EntryPoint = "gil_abi_version")]
    private static partial uint LibraryAbiVersion();

    [LibraryImport(Library, EntryPoint = "gil_last_error")]
    private static partial nuint LastError(byte* buffer, nuint capacity);

    [LibraryImport(Library, EntryPoint = "gil_last_result")]
    private static partial nuint LastResult(byte* buffer, nuint capacity);

    /// <summary>Makes sure the native library speaks the interface this binding was built for — once, before the first
    /// object is made.</summary>
    /// <exception cref="InvalidOperationException">The library speaks another version.</exception>
    /// <exception cref="DllNotFoundException">No library for the running platform was found.</exception>
    internal static void EnsureCompatible()
    {
        if (_compatible)
        {
            return;
        }

        uint found;
        try
        {
            found = LibraryAbiVersion();
        }
        catch (DllNotFoundException e)
        {
            throw NotFound(e);
        }

        if (found != AbiVersion)
        {
            throw new InvalidOperationException(
                $"The native library {Library} speaks version {found} of its interface; this binding needs version " +
                $"{AbiVersion}. Use the native library built with this binding.");
        }

        _compatible = true;
    }

    /// <summary>The runtimes whose native library the Gil package carries.</summary>
    internal static readonly string[] SupportedRuntimes =
        ["linux-x64", "linux-arm64", "osx-x64", "osx-arm64", "win-x64", "win-arm64"];

    /// <summary>The library was not found: says which platform asked and which ones the package carries it for.</summary>
    internal static DllNotFoundException NotFound(DllNotFoundException inner) =>
        new($"The native library {Library} was not found for {RuntimeInformation.RuntimeIdentifier}. The Gil package " +
            $"carries it for {string.Join(", ", SupportedRuntimes)}; on another platform, build it from the repository " +
            "(cargo build --release -p gil-ffi) and put it next to the application.", inner);

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
                $"The Gil core failed partway through a change; what it was changing can no longer be used: {message}"),
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
