namespace Jarvis.Desktop.Models;

public sealed class PublicUser
{
    public Guid Id { get; set; }
    public string Email { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public string Role { get; set; } = "";
    public bool IsActive { get; set; }
    public DateTimeOffset CreatedAt { get; set; }
}

public sealed class ChatMessage
{
    public Guid Id { get; set; }
    public Guid UserId { get; set; }
    public string Role { get; set; } = "";
    public string Content { get; set; } = "";
    public DateTimeOffset CreatedAt { get; set; }
    public bool IsAssistant => Role == "assistant";
    public bool IsUser => !IsAssistant;
    public string Who => IsAssistant ? "JARVIS" : "TY";
}

public sealed class AppLaunch
{
    public string App { get; set; } = "";
    public string Status { get; set; } = "";
    public string? Error { get; set; }
}

public sealed class SendMessageResponse
{
    public ChatMessage UserMessage { get; set; } = new();
    public ChatMessage AssistantMessage { get; set; } = new();
    public string Model { get; set; } = "";
    public List<AppLaunch> Launches { get; set; } = [];
}

public sealed class VoiceResponse
{
    public ChatMessage UserMessage { get; set; } = new();
    public ChatMessage AssistantMessage { get; set; } = new();
    public string Model { get; set; } = "";
    public string? AudioBase64 { get; set; }
    public string? AudioMime { get; set; }
    public string? SpeechError { get; set; }
    public List<AppLaunch> Launches { get; set; } = [];
}

public sealed class UsageSnapshot
{
    public long UserRpmUsed { get; set; }
    public int UserRpmLimit { get; set; }
    public long UserDailyUsed { get; set; }
    public int UserDailyLimit { get; set; }

    public string Format() =>
        $"{UserRpmUsed}/{UserRpmLimit} RPM · {UserDailyUsed}/{UserDailyLimit} dziennie";
}

public sealed class JarvisVersion
{
    public string BrainVersion { get; set; } = "";
    public string SystemName { get; set; } = "";
    public string SystemVersion { get; set; } = "";

    public string HostLabel()
    {
        var label = string.Join(' ', new[] { SystemName, SystemVersion }.Where(part => !string.IsNullOrWhiteSpace(part)));
        return string.IsNullOrWhiteSpace(label) ? "—" : label;
    }
}

public sealed class FeatureFlag
{
    public string Key { get; set; } = "";
    public bool Enabled { get; set; }
    public string Description { get; set; } = "";
    public DateTimeOffset UpdatedAt { get; set; }
}

public sealed class LoginResponse
{
    public string AccessToken { get; set; } = "";
    public PublicUser User { get; set; } = new();
}

public sealed record LoginRequest(string Email, string Password);

public sealed class ClaimLaunchesRequest
{
    public string Fingerprint { get; set; } = "";
}

public sealed class ClaimedLaunch
{
    public Guid Id { get; set; }
    public string App { get; set; } = "";
}

public sealed class ClaimLaunchesResponse
{
    public List<ClaimedLaunch> Launches { get; set; } = [];
    public string? Handoff { get; set; }
}

public sealed class SendMessageBody
{
    public string Content { get; set; } = "";
    public string? DeviceFingerprint { get; set; }
}

public sealed class DeviceHeartbeat
{
    public string Name { get; set; } = "";
    public string Platform { get; set; } = "desktop";
    public string UiVersion { get; set; } = "";
    public string ClientVersion { get; set; } = "";
    public string SystemName { get; set; } = "";
    public string SystemVersion { get; set; } = "";
    public string Fingerprint { get; set; } = "";
}

public sealed class ApiErrorBody
{
    public string? Error { get; set; }
    public string? Message { get; set; }
    public int RetryAfterSec { get; set; }
    public string? Scope { get; set; }
}
