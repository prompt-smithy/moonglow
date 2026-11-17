using System.Collections.Concurrent;

namespace Moonglow.Services
{
    public sealed class MonitorService
    {
        public IReadOnlyList<FakeMonitor> Monitors { get; set; } = new[] { new FakeMonitor("A"), new FakeMonitor("B") };
        public IReadOnlyList<FakeMonitor> Snapshot() => Monitors;
        public void ForEachDdcMonitor(Action<FakeMonitor> apply)
        {
            foreach (var monitor in Monitors.Where(m => m.SupportsDdcBrightness)) apply(monitor);
        }
    }
    public sealed record FakeMonitor(string DeviceName, bool SupportsDdcBrightness = false, uint MinBrightness = 0, uint MaxBrightness = 100, IntPtr PhysicalHandle = default);
}

namespace Moonglow.Native
{
    // The production worker is source-linked so tests exercise its actual thread,
    // retry and shutdown paths. These replace only the native API boundary.
    internal static class NativeMethods
    {
        public struct RAMP { public ushort[] Red, Green, Blue; }
        public struct MAGCOLOREFFECT { public float[] Transform; }
        public static Func<IntPtr, bool> OnWrite = _ => true;
        public static Func<IntPtr, RAMP, bool> OnRampWrite = (_, _) => true;
        public static ConcurrentQueue<(IntPtr Device, ushort Blue, long Timestamp)> Writes = new();
        public static Func<MAGCOLOREFFECT, bool> OnMagnificationWrite = _ => true;
        public static ConcurrentQueue<(float Green, float Blue, long Timestamp, bool Applied)> ColorEffects = new();
        public static ConcurrentQueue<int> NativeThreads = new();
        public static int MagnificationWrites;
        public static int DeletedContexts;
        public static int GammaReads;
        public static bool SetMonitorBrightness(IntPtr monitor, uint value) => true;

        public static void Reset()
        {
            OnWrite = _ => true;
            OnRampWrite = (_, _) => true;
            OnMagnificationWrite = _ => true;
            Writes.Clear();
            ColorEffects.Clear();
            NativeThreads.Clear();
            MagnificationWrites = DeletedContexts = GammaReads = 0;
        }
        public static IntPtr CreateDC(string? driver, string? device, string? output, IntPtr data) => device == "A" ? new IntPtr(1) : new IntPtr(2);
        public static IntPtr GetDC(IntPtr value) => new(3);
        public static int ReleaseDC(IntPtr a, IntPtr b) => 0;
        public static bool DeleteDC(IntPtr value) { Interlocked.Increment(ref DeletedContexts); return true; }
        public static bool GetDeviceGammaRamp(IntPtr device, ref RAMP ramp)
        {
            Interlocked.Increment(ref GammaReads);
            for (int i = 0; i < 256; i++) ramp.Red[i] = ramp.Green[i] = ramp.Blue[i] = (ushort)(i * 257);
            return true;
        }
        public static bool SetDeviceGammaRamp(IntPtr device, ref RAMP ramp)
        {
            NativeThreads.Enqueue(Environment.CurrentManagedThreadId);
            bool ok = OnWrite(device) && OnRampWrite(device, ramp);
            Writes.Enqueue((device, ramp.Blue[255], System.Diagnostics.Stopwatch.GetTimestamp()));
            return ok;
        }
        public static bool MagInitialize() { NativeThreads.Enqueue(Environment.CurrentManagedThreadId); return true; }
        public static bool MagUninitialize() { NativeThreads.Enqueue(Environment.CurrentManagedThreadId); return true; }
        public static bool MagSetFullscreenColorEffect(ref MAGCOLOREFFECT effect)
        {
            NativeThreads.Enqueue(Environment.CurrentManagedThreadId);
            bool applied = OnMagnificationWrite(effect);
            ColorEffects.Enqueue((effect.Transform[6], effect.Transform[12], System.Diagnostics.Stopwatch.GetTimestamp(), applied));
            Interlocked.Increment(ref MagnificationWrites);
            return applied;
        }
    }
}
