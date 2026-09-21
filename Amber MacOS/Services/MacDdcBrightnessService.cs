using System.Runtime.InteropServices;

namespace Amber.MacOS.Services;

public sealed class MacDdcBrightnessService : IBrightnessService, IDisposable
{
    private const uint DdcChipAddress = 0x37;
    private const uint DdcDataAddress = 0x51;
    private const int DdcDelayMilliseconds = 50;
    private readonly object _sync = new();
    private readonly Timer _timer;
    private IntPtr _service;
    private int _pendingBrightness = 100;
    private bool _disposed;

    public MacDdcBrightnessService()
    {
        _service = OperatingSystem.IsMacOS() && RuntimeInformation.ProcessArchitecture == Architecture.Arm64
            ? MacNative.IOAVServiceCreate(IntPtr.Zero)
            : IntPtr.Zero;

        if (_service != IntPtr.Zero && !ProbeBrightness())
        {
            MacNative.CFRelease(_service);
            _service = IntPtr.Zero;
        }

        _timer = new Timer(ApplyPendingBrightness);
    }

    public bool IsSupported => _service != IntPtr.Zero;

    public void SetBrightness(int percent)
    {
        if (!IsSupported)
            return;

        Volatile.Write(ref _pendingBrightness, Math.Clamp(percent, 1, 100));
        _timer.Change(TimeSpan.FromMilliseconds(40), Timeout.InfiniteTimeSpan);
    }

    public static byte[] BuildWritePacket(int percent)
    {
        ushort value = (ushort)Math.Clamp(percent, 1, 100);
        var packet = new byte[] { 0x84, 0x03, 0x10, (byte)(value >> 8), (byte)value, 0 };
        packet[5] = (byte)(0x6e ^ DdcDataAddress ^ packet[0] ^ packet[1] ^ packet[2] ^ packet[3] ^ packet[4]);
        return packet;
    }

    private bool ProbeBrightness()
    {
        var request = new byte[] { 0x82, 0x01, 0x10, 0xfd };
        for (int attempt = 0; attempt < 2; attempt++)
        {
            Thread.Sleep(DdcDelayMilliseconds);
            if (MacNative.IOAVServiceWriteI2C(_service, DdcChipAddress, DdcDataAddress, request, (uint)request.Length) != 0)
                return false;
        }

        Thread.Sleep(DdcDelayMilliseconds);
        var reply = new byte[12];
        return MacNative.IOAVServiceReadI2C(_service, DdcChipAddress, DdcDataAddress, reply, (uint)reply.Length) == 0;
    }

    private void ApplyPendingBrightness(object? state)
    {
        lock (_sync)
        {
            if (_disposed || _service == IntPtr.Zero)
                return;

            byte[] packet = BuildWritePacket(Volatile.Read(ref _pendingBrightness));
            for (int attempt = 0; attempt < 2; attempt++)
            {
                Thread.Sleep(DdcDelayMilliseconds);
                if (MacNative.IOAVServiceWriteI2C(_service, DdcChipAddress, DdcDataAddress, packet, (uint)packet.Length) != 0)
                    return;
            }
        }
    }

    public void Dispose()
    {
        lock (_sync)
        {
            if (_disposed)
                return;

            _disposed = true;
            _timer.Dispose();
            if (_service != IntPtr.Zero)
            {
                MacNative.CFRelease(_service);
                _service = IntPtr.Zero;
            }
        }
    }
}