using System.Text.Json;

namespace Gil.Memory;

/// <summary>
/// Memory that needs no model: cosine nearest neighbour over character n-gram TF-IDF, one index per task. Text is
/// compared by its characters, so it works without a tokenizer in any script, and a lookup costs no call (its energy is
/// 0). It is the decision core's similar record memory (crate <c>gil</c>, through the native library), the one a
/// <see cref="Decider"/> holds; texts are normalised and cut into n-grams as docs/text-normalization.md specifies. Like
/// <see cref="EmbeddingMemory"/> it holds only confirmed answers and is rebuilt from the feedback history — through
/// <see cref="MemoryReplay"/>.
/// </summary>
/// <remarks>
/// Similarities are on a different scale from embedding cosines — text that means the same but shares few characters
/// scores low — so a task's <see cref="TaskPolicy.MemoryThreshold"/> has to be chosen for this memory. Weights follow the
/// remembered answers: the inverse document frequencies are taken afresh whenever the number of remembered answers has
/// moved by a tenth (or by one, while there are fewer than ten) since they were last taken, and every row and every
/// lookup is weighted with the same ones in between — so a similarity stays a cosine, 1 for the same text, and changes in
/// steps rather than on every answer remembered. Lookups may run from several threads at once; remembering and
/// forgetting wait for them and run alone.
/// </remarks>
[Experimental(Stability.Memory, UrlFormat = Stability.Url)]
public sealed class LexicalMemory : IMemory, IDisposable
{
    private readonly ReaderWriterLockSlim _gate = new();
    private readonly LexicalMemoryHandle _handle;

    /// <param name="minGram">The shortest character n-gram.</param>
    /// <param name="maxGram">The longest character n-gram.</param>
    public unsafe LexicalMemory(int minGram = 2, int maxGram = 3)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(minGram, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(maxGram, minGram);
        Native.EnsureCompatible();
        nint output;
        Native.Check(Native.LexicalMemoryNew((nuint)minGram, (nuint)maxGram, &output));
        _handle = LexicalMemoryHandle.Of(output);
    }

    /// <summary>How many confirmed answers the task's memory holds.</summary>
    public unsafe int Count(string task)
    {
        ArgumentNullException.ThrowIfNull(task);
        var name = new Utf8(task);
        nuint count;
        _gate.EnterReadLock();
        try
        {
            fixed (byte* t = &name.Pin())
            {
                Native.Check(Native.LexicalMemoryCount(_handle, name.At(t), &count));
            }
        }
        finally
        {
            _gate.ExitReadLock();
        }

        return (int)count;
    }

    public Task<(MemoryMatch? Match, double Energy)> LookupAsync(string task, string state, string traceId, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var found = Nearest(task, state, 1);
        return Task.FromResult<(MemoryMatch?, double)>((found.Count == 0 ? null : found[0], 0));
    }

    /// <summary>
    /// The <paramref name="count"/> most similar remembered requests, most similar first; on a tie, the one remembered
    /// first. The first is the match <see cref="LookupAsync"/> returns. Empty when the task's memory is empty.
    /// </summary>
    /// <param name="task">The task whose memory to search.</param>
    /// <param name="state">The request.</param>
    /// <param name="count">At most this many; fewer when memory holds fewer.</param>
    public unsafe IReadOnlyList<MemoryMatch> Nearest(string task, string state, int count)
    {
        ArgumentNullException.ThrowIfNull(task);
        ArgumentNullException.ThrowIfNull(state);
        ArgumentOutOfRangeException.ThrowIfLessThan(count, 1);
        var name = new Utf8(task);
        var text = new Utf8(state);
        string json;
        _gate.EnterReadLock();
        try
        {
            fixed (byte* t = &name.Pin(), s = &text.Pin())
            {
                nuint length;
                json = Native.Result(Native.LexicalMemoryNearest(_handle, name.At(t), text.At(s), (nuint)count, &length));
            }
        }
        finally
        {
            _gate.ExitReadLock();
        }

        using var document = JsonDocument.Parse(json);
        return
        [
            .. document.RootElement.EnumerateArray().Select(m => new MemoryMatch(
                m.GetProperty("key").GetString()!, m.GetProperty("similarity").GetDouble(), m.GetProperty("answer").GetString()!)),
        ];
    }

    public Task<(IReadOnlyList<MemoryMatch> Matches, double Energy)> NearestAsync(string task, string state, int count, string traceId, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult<(IReadOnlyList<MemoryMatch>, double)>((Nearest(task, state, count), 0));
    }

    public unsafe Task<double> RememberAsync(string task, string key, string state, string answer, string traceId, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ArgumentNullException.ThrowIfNull(task);
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(state);
        ArgumentNullException.ThrowIfNull(answer);
        var name = new Utf8(task);
        var k = new Utf8(key);
        var text = new Utf8(state);
        var a = new Utf8(answer);
        _gate.EnterWriteLock();
        try
        {
            fixed (byte* t = &name.Pin(), kp = &k.Pin(), s = &text.Pin(), ap = &a.Pin())
            {
                Native.Check(
                    Native.LexicalMemoryRemember(_handle, name.At(t), k.At(kp), text.At(s), a.At(ap)), changing: true);
            }
        }
        finally
        {
            _gate.ExitWriteLock();
        }

        return Task.FromResult(0.0);
    }

    public unsafe void Forget(string task, string key)
    {
        ArgumentNullException.ThrowIfNull(task);
        ArgumentNullException.ThrowIfNull(key);
        var name = new Utf8(task);
        var k = new Utf8(key);
        _gate.EnterWriteLock();
        try
        {
            fixed (byte* t = &name.Pin(), kp = &k.Pin())
            {
                Native.Check(Native.LexicalMemoryForget(_handle, name.At(t), k.At(kp)), changing: true);
            }
        }
        finally
        {
            _gate.ExitWriteLock();
        }
    }

    /// <summary>Frees the native memory; a later call is refused.</summary>
    public void Dispose()
    {
        _handle.Dispose();
        _gate.Dispose();
    }
}
