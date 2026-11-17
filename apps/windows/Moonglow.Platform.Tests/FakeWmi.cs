using System.Collections;

// Only the native WMI boundary is replaced. The production detection and
// publication logic is source-linked into the portable platform tests.
namespace System.Management;

public sealed class ManagementObject : IDisposable
{
    public object this[string name] => name == "Active" ? true : 80;
    public object? InvokeMethod(string name, object[] args) => null;
    public void Dispose() { }
}

public sealed class ManagementObjectSearcher : IDisposable
{
    public static Func<ManagementObject[]> Query = () => Array.Empty<ManagementObject>();
    public ManagementObjectSearcher(string scope, string query) { }
    public ManagementObjectCollection Get() => new(Query());
    public void Dispose() { }
}

public sealed class ManagementObjectCollection : IEnumerable, IDisposable
{
    private readonly ManagementObject[] _objects;
    public ManagementObjectCollection(ManagementObject[] objects) => _objects = objects;
    public IEnumerator GetEnumerator() => _objects.GetEnumerator();
    public void Dispose() { }
}
