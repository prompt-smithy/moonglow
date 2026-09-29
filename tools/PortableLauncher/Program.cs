using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;

namespace Amber.PortableLauncher;

internal static class Program
{
    private const string PayloadPrefix = "AmberPortable.Payload.";
    private const uint MessageBoxError = 0x10;

    [STAThread]
    private static int Main()
    {
        string extractionDirectory = Path.Combine(
            Path.GetTempPath(),
            "AmberPortable-" + Guid.NewGuid().ToString("N"));

        try
        {
            ExtractPayload(extractionDirectory);

            var startInfo = new ProcessStartInfo(Path.Combine(extractionDirectory, "Amber.exe"))
            {
                WorkingDirectory = extractionDirectory,
                UseShellExecute = true,
            };

            using Process app = Process.Start(startInfo)
                ?? throw new InvalidOperationException("Windows could not start Amber.");
            app.WaitForExit();
            return app.ExitCode;
        }
        catch (Exception exception)
        {
            ShowMessage(IntPtr.Zero, "Amber Portable could not start.\r\n\r\n" + exception.Message,
                "Amber Portable", MessageBoxError);
            return 1;
        }
        finally
        {
            try
            {
                if (Directory.Exists(extractionDirectory))
                    Directory.Delete(extractionDirectory, recursive: true);
            }
            catch
            {
            }
        }
    }

    private static void ExtractPayload(string destination)
    {
        Assembly launcher = Assembly.GetExecutingAssembly();
        string[] resources = launcher.GetManifestResourceNames()
            .Where(name => name.StartsWith(PayloadPrefix, StringComparison.Ordinal))
            .ToArray();
        if (resources.Length == 0)
            throw new InvalidOperationException("The embedded application payload is missing.");

        Directory.CreateDirectory(destination);
        foreach (string resourceName in resources)
        {
            string fileName = resourceName.Substring(PayloadPrefix.Length);
            if (Path.GetFileName(fileName) != fileName)
                throw new InvalidOperationException("The embedded payload contains an invalid file name.");

            using Stream resource = launcher.GetManifestResourceStream(resourceName)
                ?? throw new InvalidOperationException("Could not read embedded file " + fileName + ".");
            using var output = File.Create(Path.Combine(destination, fileName));
            resource.CopyTo(output);
        }

        if (!File.Exists(Path.Combine(destination, "Amber.exe")))
            throw new InvalidOperationException("The embedded application executable is missing.");
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "MessageBoxW")]
    private static extern int ShowMessage(IntPtr window, string text, string caption, uint type);
}