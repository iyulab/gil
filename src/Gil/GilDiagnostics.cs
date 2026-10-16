using System.Diagnostics;
using System.Diagnostics.Metrics;
using System.Reflection;

namespace Gil;

/// <summary>
/// The traces and metrics Gil emits through the BCL: register <see cref="Name"/> as an activity source and a meter
/// (for OpenTelemetry, <c>AddSource(GilDiagnostics.Name)</c> and <c>AddMeter(GilDiagnostics.Name)</c>).
/// </summary>
/// <remarks>
/// <para>Spans: <c>gil.call</c> per model call (role, node, layer, model, tokens, energy) and, from the tree judgment
/// path (package <c>Gil.Tree</c>), <c>gil.resolve</c> per request (task, mode, memory outcome, energy, confidence,
/// failure) with its calls under it.</para>
/// <para>Metrics: <c>gil.call.energy</c> (by role) and, from the tree judgment path, <c>gil.resolutions</c> (count by
/// task, mode and memory outcome), <c>gil.resolution.energy</c> and <c>gil.resolution.duration</c> (by task and mode).
/// Energy is in the unit the energy models report.</para>
/// <para>Only what Gil itself knows is emitted. Provider-level telemetry (<c>gen_ai.*</c> spans and token metrics)
/// belongs to the model client — IronHive, or Microsoft.Extensions.AI's <c>UseOpenTelemetry</c> — and nests under
/// <c>gil.call</c> when that client emits it, so tokens are not counted twice.</para>
/// </remarks>
public static class GilDiagnostics
{
    /// <summary>The name of Gil's activity source and meter.</summary>
    public const string Name = "Gil";

    private static readonly string? Version = typeof(GilDiagnostics).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;

    internal static readonly ActivitySource Source = new(Name, Version);

    private static readonly Meter Meter = new(Name, Version);

    private static readonly Histogram<double> CallEnergy = Meter.CreateHistogram<double>("gil.call.energy", "{energy}", "Energy per model call, in the energy model's unit.");

    internal static Activity? StartCall(string role, string traceId, string? nodeId, int? layer)
    {
        var activity = Source.StartActivity("gil.call");
        activity?.SetTag("gil.call.role", role);
        activity?.SetTag("gil.trace_id", traceId);
        activity?.SetTag("gil.node", nodeId);
        activity?.SetTag("gil.layer", layer);
        return activity;
    }

    internal static void Called(Activity? activity, CallRecord call)
    {
        CallEnergy.Record(call.Energy, new KeyValuePair<string, object?>("gil.call.role", call.Role));
        if (activity is null)
        {
            return;
        }

        activity.SetTag("gil.call.model", call.Model);
        activity.SetTag("gil.call.input_tokens", call.PromptTokens);
        activity.SetTag("gil.call.cached_tokens", call.CachedTokens);
        activity.SetTag("gil.call.output_tokens", call.CompletionTokens);
        activity.SetTag("gil.energy", call.Energy);
    }

    internal static void Failed(Activity? activity, Exception error)
    {
        activity?.SetStatus(ActivityStatusCode.Error, error.Message);
        activity?.SetTag("error.type", error.GetType().FullName);
        activity?.AddException(error);
    }
}
