namespace Moonglow.Core;

/// <summary>Second-instance detection that does not depend on kernel namespaces.</summary>
/// <remarks>
/// A named mutex alone proved insufficient in practice: two same-generation builds were
/// observed owning the display gamma ramps at once (their log lines interleaved in one
/// file within the same second). A process scan backs the mutex up.
/// </remarks>
public static class InstanceGuard
{
    /// <summary>Executable-name prefixes owned by this app family, current and pre-merge.</summary>
    public static IReadOnlyList<string> OwnPrefixes { get; } = new[] { "Moonglow", "Amber" };

    /// <summary>Matches the family name itself and suffixed variants
    /// (<c>Moonglow_Portable</c>), but not unrelated names that merely share the
    /// beginning (<c>AmberAlert</c>).</summary>
    private static bool IsFamilyName(string name, string prefix)
    {
        if (name.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
        {
            if (name.Length == prefix.Length)
                return true;

            char next = name[prefix.Length];
            if (next == '_' || next == '.' || next == '-')
                return true;
        }

        return false;
    }

    /// <summary>Whether any process other than this copy looks like another copy of
    /// this app. The direct parent is exempt: that is the portable launcher that
    /// started this process, not a rival.</summary>
    public static bool IsAnotherCopyRunning(
        IReadOnlyList<(string Name, int Pid)> processes,
        int selfPid,
        IReadOnlyList<string> ownPrefixes,
        int? parentPid = null)
    {
        foreach (var (name, pid) in processes)
        {
            if (pid == selfPid || pid == parentPid)
                continue;

            foreach (string prefix in ownPrefixes)
            {
                if (IsFamilyName(name, prefix))
                    return true;
            }
        }

        return false;
    }
}
