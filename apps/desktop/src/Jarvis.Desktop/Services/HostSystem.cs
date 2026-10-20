namespace Jarvis.Desktop.Services;

public static class HostSystem
{
    public static (string Name, string Version) Describe()
    {
        if (OperatingSystem.IsWindows())
            return ("Windows", Sanitize(Environment.OSVersion.Version.ToString(3)));
        if (OperatingSystem.IsMacOS())
            return ("macOS", Sanitize(Environment.OSVersion.Version.ToString(3)));
        if (OperatingSystem.IsLinux())
            return ("Linux", Sanitize(Environment.OSVersion.Version.ToString()));
        return ("unknown", "");
    }

    public static string DeviceName()
    {
        var host = Environment.MachineName.Trim();
        if (string.IsNullOrWhiteSpace(host))
            host = "desktop";
        var name = $"{host} · desktop";
        return name.Length <= 80 ? name : name[..80];
    }

    private static string Sanitize(string value)
    {
        var filtered = new string(value.Where(c => char.IsAsciiLetterOrDigit(c) || c is '.' or '-' or '_' or '+').ToArray());
        return filtered.Length <= 40 ? filtered : filtered[..40];
    }
}
