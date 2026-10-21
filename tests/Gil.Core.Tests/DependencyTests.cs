using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>The decision core (package Gil) stands alone: the experimental tracks and the model ports build on it, never
/// the other way round.</summary>
public sealed class DependencyTests
{
    [Fact]
    public void The_core_assembly_references_neither_the_experimental_tracks_nor_their_dependencies()
    {
        var references = typeof(Decider).Assembly.GetReferencedAssemblies().Select(a => a.Name).ToList();

        references.Should().NotContain(["Gil.Tree", "Gil.Abstractions", "YamlDotNet", "Microsoft.Extensions.AI", "Microsoft.Extensions.AI.Abstractions"]);
        typeof(Decider).Assembly.GetName().Name.Should().Be("Gil");
    }
}
