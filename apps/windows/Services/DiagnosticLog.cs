using System.Collections.Concurrent;
using System.IO;
using System.Threading;

namespace Moonglow.Services;

/// <summary>Bounded background diagnostics; slider and display workers never wait for disk.</summary>
internal static class DiagnosticLog
{
    private const long MaxBytes = 512 * 1024;
    private static readonly BlockingCollection<string> Pending = new(256);
    private static readonly int ProcessId = System.Diagnostics.Process.GetCurrentProcess().Id;

    static DiagnosticLog()
    {
        new Thread(WriteLoop) { IsBackground = true, Name = "DiagnosticLog" }.Start();
    }

    public static void Write(string message)
    {
        Pending.TryAdd($"{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff} [p{ProcessId}/t{Environment.CurrentManagedThreadId}] {message}");
    }

    private static void WriteLoop()
    {
        string path = Path.Combine(Path.GetTempPath(), "Moonglow-color.log");
        foreach (string line in Pending.GetConsumingEnumerable())
        {
            try
            {
                if (File.Exists(path) && new FileInfo(path).Length >= MaxBytes)
                {
                    File.Delete(path + ".3");
                    for (int index = 2; index >= 1; index--)
                        if (File.Exists(path + "." + index))
                            File.Move(path + "." + index, path + "." + (index + 1));
                    File.Move(path, path + ".1");
                }
                File.AppendAllText(path, line + Environment.NewLine);
            }
            catch { /* Diagnostics are best effort. */ }
        }
    }
}
