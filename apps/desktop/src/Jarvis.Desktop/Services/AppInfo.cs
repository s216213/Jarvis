using System.Reflection;

namespace Jarvis.Desktop.Services;

public static class AppInfo
{
    public const string UiVersion = "future-ui-v1";

    public static string ClientVersion { get; } = ReadVersion();

    private static string ReadVersion()
    {
        var info = typeof(AppInfo).Assembly
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
            ?? "0.1.0";
        var plus = info.IndexOf('+');
        return plus >= 0 ? info[..plus] : info;
    }
}
