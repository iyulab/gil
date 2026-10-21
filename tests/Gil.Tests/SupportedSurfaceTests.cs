using AwesomeAssertions;
using SupportedSurface;

namespace Gil.Tests;

/// <summary>The walkthrough compiled with no experimental diagnostic suppressed (tests/SupportedSurface) also runs.</summary>
public sealed class SupportedSurfaceTests
{
    [Fact]
    public async Task The_walkthrough_of_the_supported_surface_runs()
    {
        (await FormsWalkthrough.RunAsync()).Should().BeGreaterThan(3);
    }
}
