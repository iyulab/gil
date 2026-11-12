using System.Text.RegularExpressions;
using AwesomeAssertions;
using Gil.Review;

namespace Gil.Review.Tests;

/// <summary>The README's example is compiled and run here, so it breaks together with the API: the test holds the
/// README's code as a method, checks that every line of the README's code block is in it in the same order, and runs
/// it.</summary>
public sealed partial class ReadmeTests
{
    // The code of the README's code block, line for line (a using directive can only open a file, so it is at the top).
    private static void Example()
    {
        var sample = ReviewSampler.DrawPilot(
            [new PopulationStratum("answered", ["d1", "d2", "d3"]), new PopulationStratum("abstained", ["d4", "d5"])],
            perStratum: 30,
            seed: 7);
        // Both strata are smaller than the pilot: taken whole.
        Console.WriteLine(sample.SelectedCount); // 5
    }

    [Fact]
    public void The_readme_code_is_the_compiled_example()
    {
        var readme = File.ReadAllText(Path.Combine(Root(), "Gil.Review", "README.md")).ReplaceLineEndings("\n");
        var blocks = CodeBlock().Matches(readme).Select(m => Lines(m.Groups[1].Value)).ToList();
        blocks.Should().NotBeEmpty();

        var source = Lines(File.ReadAllText(SourceFile()));
        foreach (var directive in blocks.SelectMany(b => b).Where(l => UsingDirective().IsMatch(l)))
        {
            source.Should().Contain(directive, "a namespace the README's code imports should be imported by this file");
        }

        var at = source.FindIndex(l => l == "private static void Example()");
        foreach (var line in blocks.SelectMany(b => b).Where(l => !UsingDirective().IsMatch(l)))
        {
            var found = source.FindIndex(at, l => l == line);
            found.Should().BeGreaterThanOrEqualTo(0, $"README line \"{line}\" should appear in Example() after line {at}");
            at = found + 1;
        }
    }

    [Fact]
    public void The_readme_example_runs_and_prints_what_the_comment_says()
    {
        var printed = new StringWriter();
        var console = Console.Out;
        Console.SetOut(printed);
        try
        {
            Example();
        }
        finally
        {
            Console.SetOut(console);
        }

        printed.ToString().Trim().Should().Be("5");
    }

    private static List<string> Lines(string text) =>
        [.. text.ReplaceLineEndings("\n").Split('\n').Select(l => l.Trim()).Where(l => l.Length > 0)];

    private static string SourceFile() => Path.Combine(Root(), "Gil.Review.Tests", "ReadmeTests.cs");

    private static string Root()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null && !File.Exists(Path.Combine(directory.FullName, "Gil.Review.slnx")))
        {
            directory = directory.Parent;
        }

        return directory?.FullName ?? throw new InvalidOperationException("Gil.Review.slnx not found above the test output");
    }

    [GeneratedRegex(@"```csharp\n(.*?)```", RegexOptions.Singleline)]
    private static partial Regex CodeBlock();

    [GeneratedRegex(@"^using [A-Za-z_][A-Za-z0-9_.]*;$")]
    private static partial Regex UsingDirective();
}
