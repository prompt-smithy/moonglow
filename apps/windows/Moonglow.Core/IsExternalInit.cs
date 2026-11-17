#if NETFRAMEWORK
namespace System.Runtime.CompilerServices;

// Records are part of the portable contract; .NET Framework 4.8 predates
// the compiler marker used for their init-only properties.
internal static class IsExternalInit { }
#endif
