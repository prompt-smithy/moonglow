namespace System.Runtime.CompilerServices;

/// <summary>
/// The compiler requires this type to exist in order to allow C# 9 <c>init</c>-only setters.
/// It is part of modern .NET but not of .NET Framework 4.8, so we declare it ourselves.
/// </summary>
internal static class IsExternalInit
{
}
