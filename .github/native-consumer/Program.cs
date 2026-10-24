// A consumer of the built Gil, Gil.Review and Gil.Tree packages, run by run.sh where the .NET host picks each
// native library: its first call checks the interface version.

using Gil;

var floor = Gil.Review.ReviewSampler.MinimumDesignDeviation;
var counts = Gil.Review.ReviewSampler.AllocateNeyman([new(600, 0.5), new(400, 0.1)], 50);
Console.WriteLine($"floor {floor:F6} · allocation {string.Join(",", counts)}");
if (Math.Abs(floor - Math.Sqrt(0.05 * 0.95)) > 1e-12 || counts.Sum() != 50)
{
    return 1;
}

using var schema = new Schema("ticket",
[
    new AttributeDefinition("component", AttributeRole.Observed),
    new AttributeDefinition("team", AttributeRole.Decided) { KeyThreshold = 0.6 },
]);
using var decider = new Decider();
for (var i = 0; i < 20; i++)
{
    using var settled = OpenRecord.Open(schema, $"t/{i}", DateTimeOffset.UnixEpoch.AddMinutes(i));
    settled.Observe(decider, "component", "vpn");
    settled.Settle(decider, "team", SettlementKind.Enter, DateTimeOffset.UnixEpoch.AddMinutes(i), value: "network");
}

using var open = OpenRecord.Open(schema, "t/new", DateTimeOffset.UnixEpoch.AddHours(1));
var team = open.Observe(decider, "component", "vpn").Single(a => a.Attribute == "team");
var top = team.Candidates.Count > 0 ? team.Candidates[0].Value : null;
Console.WriteLine($"team {top} · answered {team.Answered}");
// Gil.Tree's lexical memory runs on the same library, found through the Gil package.
#pragma warning disable GIL0004
using var lexical = new Gil.Memory.LexicalMemory();
await lexical.RememberAsync("support", "t1", "refund for my last order", "refund_policy", "t1");
var near = lexical.Nearest("support", "Refund for my LAST order", 1);
#pragma warning restore GIL0004
Console.WriteLine($"lexical {near[0].Answer} · similarity {near[0].Similarity:F6}");
return team.Answered && top == "network" && near[0].Answer == "refund_policy"
    && Math.Abs(near[0].Similarity - 1.0) < 1e-12 ? 0 : 1;
