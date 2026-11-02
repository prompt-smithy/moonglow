using System.Collections.Concurrent;

namespace Moonglow.Services
{
    public sealed class MonitorService
    {
        public IReadOnlyList<FakeMonitor> Snapshot() => new[] { new FakeMonitor("A"), new FakeMonitor("B") };
    }
    public sealed record FakeMonitor(string DeviceName);
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
        public static ConcurrentQueue<(IntPtr Device, ushort Blue)> Writes = new();
        public static ConcurrentQueue<int> NativeThreads = new();
        public static int MagnificationWrites;
        public static int DeletedContexts;

        public static void Reset()
        {
            OnWrite = _ => true;
            Writes.Clear();
            NativeThreads.Clear();
            MagnificationWrites = DeletedContexts = 0;
        }
        public static IntPtr CreateDC(string? driver, string? device, string? output, IntPtr data) => device == "A" ? new IntPtr(1) : new IntPtr(2);
        public static IntPtr GetDC(IntPtr value) => new(3);
        public static int ReleaseDC(IntPtr a, IntPtr b) => 0;
        public static bool DeleteDC(IntPtr value) { Interlocked.Increment(ref DeletedContexts); return true; }
        public static bool GetDeviceGammaRamp(IntPtr device, ref RAMP ramp)
        {
            for (int i = 0; i < 256; i++) ramp.Red[i] = ramp.Green[i] = ramp.Blue[i] = (ushort)(i * 257);
            return true;
        }
        public static bool SetDeviceGammaRamp(IntPtr device, ref RAMP ramp)
        {
            NativeThreads.Enqueue(Environment.CurrentManagedThreadId);
            bool ok = OnWrite(device);
            Writes.Enqueue((device, ramp.Blue[255]));
            return ok;
        }
        public static bool MagInitialize() { NativeThreads.Enqueue(Environment.CurrentManagedThreadId); return true; }
        public static bool MagUninitialize() { NativeThreads.Enqueue(Environment.CurrentManagedThreadId); return true; }
        public static bool MagSetFullscreenColorEffect(ref MAGCOLOREFFECT effect) { Interlocked.Increment(ref MagnificationWrites); return true; }
    }
}
