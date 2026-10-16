using AwesomeAssertions;
using Gil.Forms;

namespace Gil.Tests.Forms;

/// <summary>The decision core (package Gil) stands without the tree judgment path, which lives in Gil.Tree.</summary>
public sealed class CoreDependencyTests
{
    [Fact]
    public void The_core_assembly_references_neither_the_tree_path_nor_its_yaml_reader()
    {
        var references = typeof(FormResolver).Assembly.GetReferencedAssemblies().Select(a => a.Name).ToList();

        references.Should().NotContain(["Gil.Tree", "YamlDotNet"]);
        typeof(Resolver).Assembly.GetName().Name.Should().Be("Gil.Tree");
    }
}
