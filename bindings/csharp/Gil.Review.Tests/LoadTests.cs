using System.Runtime.InteropServices;
using AwesomeAssertions;

namespace Gil.Review.Tests;

/// <summary>When the native library cannot be found, the failure says which platform asked and which ones the package
/// carries a library for.</summary>
public sealed class LoadTests
{
    [Fact]
    public void A_missing_library_names_the_running_platform_and_the_supported_ones()
    {
        var inner = new DllNotFoundException("Unable to load shared library 'gil_review_native'");

        var error = Native.NotFound(inner);

        error.Message.Should().Contain("gil_review_native").And.Contain(RuntimeInformation.RuntimeIdentifier)
            .And.Contain("linux-x64, linux-arm64, osx-x64, osx-arm64, win-x64, win-arm64")
            .And.Contain("cargo build --release -p gil-review-ffi");
        error.InnerException.Should().BeSameAs(inner);
    }
}
