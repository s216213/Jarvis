using System.Text.Json;
using Jarvis.Desktop.Models;

namespace Jarvis.Desktop.Services;

public sealed class SettingsStore
{
    private static readonly JsonSerializerOptions Json = new()
    {
        WriteIndented = true,
        PropertyNameCaseInsensitive = true,
    };

    public string ApiBase { get; set; } = "http://localhost:8080";
    public string Fingerprint { get; set; } = "";
    public string? Token { get; set; }
    public PublicUser? User { get; set; }

    public static string FilePath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Jarvis",
        "desktop-settings.json");

    public static SettingsStore Load()
    {
        try
        {
            if (File.Exists(FilePath))
            {
                var loaded = JsonSerializer.Deserialize<SettingsStore>(File.ReadAllText(FilePath), Json);
                if (loaded is not null)
                {
                    loaded.EnsureFingerprint();
                    return loaded;
                }
            }
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException)
        {
            // A broken settings file should not block a fresh login.
        }

        var fresh = new SettingsStore();
        fresh.EnsureFingerprint();
        fresh.Save();
        return fresh;
    }

    public void EnsureFingerprint()
    {
        if (string.IsNullOrWhiteSpace(Fingerprint) || Fingerprint.Length < 8)
            Fingerprint = Guid.NewGuid().ToString("D");
    }

    public void Save()
    {
        EnsureFingerprint();
        var directory = Path.GetDirectoryName(FilePath);
        if (!string.IsNullOrEmpty(directory))
            Directory.CreateDirectory(directory);
        var temp = FilePath + ".tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(this, Json));
        File.Move(temp, FilePath, overwrite: true);
    }
}
