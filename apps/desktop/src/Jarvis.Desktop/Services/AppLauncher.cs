using System.Diagnostics;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;

namespace Jarvis.Desktop.Services;

public static class AppLauncher
{
    public static void BringForward()
    {
        if (Application.Current?.ApplicationLifetime is not IClassicDesktopStyleApplicationLifetime desktop)
            return;
        if (desktop.MainWindow is not Window window)
            return;
        if (window.WindowState == WindowState.Minimized)
            window.WindowState = WindowState.Normal;
        window.Show();
        window.Activate();
    }

    public static string? LaunchAll(IEnumerable<string> apps)
    {
        var errors = new List<string>();
        foreach (var app in apps)
        {
            var error = TryLaunch(app);
            if (error is not null)
                errors.Add($"{app}: {error}");
        }

        return errors.Count == 0 ? null : string.Join(" ", errors);
    }

    public static string? TryLaunch(string app)
    {
        if (!TryNormalize(app, out var target))
            return "Niedozwolona nazwa aplikacji.";

        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = target,
                UseShellExecute = true,
            });
            return null;
        }
        catch (Exception ex)
        {
            return string.IsNullOrWhiteSpace(ex.Message)
                ? "Nie udało się uruchomić aplikacji."
                : ex.Message;
        }
    }

    private static bool TryNormalize(string raw, out string target)
    {
        target = "";
        var trimmed = raw.Trim().Trim('"').Trim();
        if (trimmed.Length == 0 || trimmed.Length > 240)
            return false;
        if (trimmed.Any(char.IsControl))
            return false;

        var lower = trimmed.ToLowerInvariant();
        if (Alias(lower) is { } aliased)
        {
            target = aliased;
            return true;
        }

        if (IsSettings(lower))
        {
            target = lower;
            return true;
        }

        if (IsHttp(trimmed))
        {
            target = trimmed;
            return true;
        }

        if (trimmed.Contains('\\') || trimmed.Contains('/'))
            return TryPath(trimmed, out target);

        if (IsBare(trimmed))
        {
            target = trimmed;
            return true;
        }

        return false;
    }

    private static string? Alias(string lower) => lower switch
    {
        "notatnik" => "notepad",
        "kalkulator" or "calculator" => "calc",
        "eksplorator" => "explorer",
        "chrome" or "google chrome" => "chrome",
        "edge" or "microsoft edge" => "msedge",
        "firefox" or "mozilla firefox" => "firefox",
        "vscode" or "visual studio code" => "code",
        "spotify" => "spotify",
        "paint" => "mspaint",
        "ustawienia" or "settings" or "ms-settings" => "ms-settings:",
        "terminal" or "windows terminal" => "wt",
        _ => null,
    };

    private static bool IsSettings(string lower)
    {
        const string prefix = "ms-settings:";
        if (!lower.StartsWith(prefix, StringComparison.Ordinal))
            return false;
        return lower[prefix.Length..].All(c => char.IsAsciiLetterOrDigit(c) || c == '-');
    }

    private static bool IsHttp(string raw)
    {
        var rest = raw.StartsWith("https://", StringComparison.OrdinalIgnoreCase) ? raw[8..]
            : raw.StartsWith("http://", StringComparison.OrdinalIgnoreCase) ? raw[7..]
            : null;
        if (string.IsNullOrEmpty(rest) || rest.StartsWith('/'))
            return false;
        return rest.All(c => char.IsAsciiLetterOrDigit(c) || ".-_~:/?&=%#+@".Contains(c));
    }

    private static bool IsBare(string raw)
    {
        if (raw.Length > 64 || !char.IsAsciiLetterOrDigit(raw[0]))
            return false;
        return raw.All(c => char.IsAsciiLetterOrDigit(c) || c is '.' or '_' or '+' or '-');
    }

    private static bool TryPath(string raw, out string target)
    {
        target = "";
        if (raw.IndexOfAny(['&', '|', ';', '`', '$', '<', '>', '"', '\'']) >= 0)
            return false;
        if (!IsLocalAbsolute(raw) || raw.Contains("..", StringComparison.Ordinal))
            return false;

        var ext = Path.GetExtension(raw).TrimStart('.').ToLowerInvariant();
        if (ext is "bat" or "cmd" or "ps1" or "vbs" or "js" or "msi" or "scr" or "com" or "jar" or "py" or "sh")
            return false;
        if (ext is not ("exe" or "lnk") && !(ext.Length == 0 && raw.StartsWith('/')))
            return false;

        target = raw;
        return true;
    }

    private static bool IsLocalAbsolute(string raw)
    {
        if (raw.Length >= 3
            && char.IsAsciiLetter(raw[0])
            && raw[1] == ':'
            && (raw[2] == '\\' || raw[2] == '/'))
            return true;
        return raw.StartsWith('/') && !raw.StartsWith("//");
    }
}
