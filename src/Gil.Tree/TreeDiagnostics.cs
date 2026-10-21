using System.Diagnostics;
using System.Diagnostics.Metrics;
using System.Reflection;

namespace Gil;

/// <summary>
/// The resolution span and metrics of the tree judgment path, under the same activity source and meter name as the call
/// spans (<see cref="GilDiagnostics.Name"/>), so one registration listens to both.
/// </summary>
internal static class TreeDiagnostics
{
    private static readonly string? Version = typeof(TreeDiagnostics).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;

    public static readonly ActivitySource Source = new(GilDiagnostics.Name, Version);

    private static readonly Meter Meter = new(GilDiagnostics.Name, Version);

    private static readonly Counter<long> Resolutions = Meter.CreateCounter<long>("gil.resolutions", "{resolution}", "Requests resolved.");

    private static readonly Histogram<double> ResolutionEnergy = Meter.CreateHistogram<double>("gil.resolution.energy", "{energy}", "Energy spent per request, in the energy models' unit.");

    private static readonly Histogram<double> ResolutionDuration = Meter.CreateHistogram<double>("gil.resolution.duration", "s", "Time to resolve a request.");

    public static void Resolved(Activity? activity, string task, Resolution resolution, string memory, TimeSpan elapsed)
    {
        var tags = new TagList { { "gil.task", task }, { "gil.mode", resolution.Mode } };
        ResolutionEnergy.Record(resolution.Energy, tags);
        ResolutionDuration.Record(elapsed.TotalSeconds, tags);
        tags.Add("gil.memory", memory);
        Resolutions.Add(1, tags);
        if (activity is null)
        {
            return;
        }

        activity.SetTag("gil.trace_id", resolution.TraceId);
        activity.SetTag("gil.mode", resolution.Mode);
        activity.SetTag("gil.memory", memory);
        activity.SetTag("gil.energy", resolution.Energy);
        activity.SetTag("gil.confidence", resolution.Confidence);
        activity.SetTag("gil.failure", resolution.Failure);
    }

    public static void Failed(Activity? activity, Exception error)
    {
        activity?.SetStatus(ActivityStatusCode.Error, error.Message);
        activity?.SetTag("error.type", error.GetType().FullName);
        activity?.AddException(error);
    }
}
