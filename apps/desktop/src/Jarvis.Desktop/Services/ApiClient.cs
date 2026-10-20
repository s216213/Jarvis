using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Jarvis.Desktop.Models;

namespace Jarvis.Desktop.Services;

public sealed class ApiException : Exception
{
    public ApiException(int status, string message) : base(message)
    {
        Status = status;
        ErrorCode = message;
    }

    public ApiException(int status, ApiErrorBody? body) : base(Describe(status, body))
    {
        Status = status;
        ErrorCode = body?.Error;
        RetryAfterSec = body?.RetryAfterSec ?? 0;
        Scope = body?.Scope;
    }

    public int Status { get; }
    public string? ErrorCode { get; }
    public int RetryAfterSec { get; }
    public string? Scope { get; }
    public bool IsRateLimited => Status == 429 || ErrorCode == "rate_limited";

    public string ForLogin()
    {
        if (IsRateLimited)
            return $"Za dużo prób logowania. Spróbuj za {(RetryAfterSec > 0 ? RetryAfterSec : 60)}s.";
        if (ErrorCode == "unauthorized")
            return "Błędny email lub hasło.";
        if (ErrorCode == "forbidden")
            return "Konto jest wyłączone.";
        return string.IsNullOrWhiteSpace(Message) ? "Logowanie nieudane." : Message;
    }

    public string ForChat()
    {
        if (IsRateLimited)
        {
            var scope = string.IsNullOrWhiteSpace(Scope) ? "limit" : Scope;
            return $"Limit AI ({scope}). Spróbuj ponownie za {(RetryAfterSec > 0 ? RetryAfterSec : 60)}s.";
        }
        return string.IsNullOrWhiteSpace(Message) ? "Wysyłanie nieudane." : Message;
    }

    public string ForVoice()
    {
        if (IsRateLimited)
            return ForChat();
        var text = Message ?? "";
        if (text.Contains("voice engine is not configured", StringComparison.OrdinalIgnoreCase))
            return "Silnik głosu na serwerze nie jest skonfigurowany.";
        if (text.Contains("no speech recognized", StringComparison.OrdinalIgnoreCase))
            return "Nie rozpoznano mowy.";
        if (text.Contains("speech service unavailable", StringComparison.OrdinalIgnoreCase)
            || text.Contains("speech synthesis unavailable", StringComparison.OrdinalIgnoreCase))
            return "Silnik głosu nie odpowiada.";
        if (text.Contains("voice is disabled", StringComparison.OrdinalIgnoreCase))
            return "Tryb głosowy jest wyłączony.";
        if (text.Contains("audio must be", StringComparison.OrdinalIgnoreCase)
            || text.Contains("audio exceeds", StringComparison.OrdinalIgnoreCase))
            return "Nagranie musi być krótkie, 16 kHz, mono.";
        if (text.Contains("unknown voice", StringComparison.OrdinalIgnoreCase))
            return "Wybrany głos nie jest dostępny.";
        return ForChat();
    }

    private static string Describe(int status, ApiErrorBody? body)
    {
        if (!string.IsNullOrWhiteSpace(body?.Message))
            return body.Message;
        if (!string.IsNullOrWhiteSpace(body?.Error))
            return body.Error;
        return status == 0 ? "Nie można połączyć z API." : $"HTTP {status}";
    }
}

public sealed class ApiClient
{
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true,
    };

    private readonly HttpClient _http = new(new SocketsHttpHandler
    {
        PooledConnectionLifetime = TimeSpan.FromMinutes(2),
    })
    {
        Timeout = TimeSpan.FromMinutes(3),
        // HTTPS defaults to HTTP/2. A long Gemini reply was saved by the API, but HttpClient
        // never finished reading that stream, so the chat stayed on "thinking" until restart.
        DefaultRequestVersion = HttpVersion.Version11,
        DefaultVersionPolicy = HttpVersionPolicy.RequestVersionExact,
    };

    public string BaseUrl { get; private set; } = "http://localhost:8080";

    public void UseBase(string raw) => BaseUrl = Normalize(raw);

    public static bool TryNormalize(string raw, out string normalized, out string error)
    {
        try
        {
            normalized = Normalize(raw);
            error = "";
            return true;
        }
        catch (Exception ex)
        {
            normalized = "";
            error = ex.Message;
            return false;
        }
    }

    public static string Normalize(string raw)
    {
        var value = (raw ?? "").Trim().TrimEnd('/');
        if (value.Length == 0)
            throw new InvalidOperationException("Podaj adres API, na przykład http://localhost:8080.");
        if (!value.Contains("://", StringComparison.Ordinal))
            value = "http://" + value;
        if (!Uri.TryCreate(value, UriKind.Absolute, out var uri)
            || (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps))
        {
            throw new InvalidOperationException("Adres API musi zaczynać się od http:// lub https://.");
        }

        var path = uri.AbsolutePath.TrimEnd('/');
        return uri.GetLeftPart(UriPartial.Authority) + path;
    }

    public Task<LoginResponse> LoginAsync(string email, string password, CancellationToken ct = default) =>
        SendAsync<LoginResponse>(HttpMethod.Post, "/auth/login", null, new LoginRequest(email, password), ct);

    public Task<PublicUser> GetMeAsync(string token, CancellationToken ct = default) =>
        SendAsync<PublicUser>(HttpMethod.Get, "/me", token, null, ct);

    public Task<JarvisVersion> GetVersionAsync(string token, CancellationToken ct = default) =>
        SendAsync<JarvisVersion>(HttpMethod.Get, "/version", token, null, ct);

    public Task<List<ChatMessage>> GetMessagesAsync(string token, CancellationToken ct = default) =>
        SendAsync<List<ChatMessage>>(HttpMethod.Get, "/chat/messages", token, null, ct);

    public Task<UsageSnapshot> GetUsageAsync(string token, CancellationToken ct = default) =>
        SendAsync<UsageSnapshot>(HttpMethod.Get, "/me/usage", token, null, ct);

    public Task<SendMessageResponse> SendMessageAsync(
        string token,
        string content,
        string fingerprint,
        CancellationToken ct = default) =>
        SendAsync<SendMessageResponse>(HttpMethod.Post, "/chat/messages", token, new SendMessageBody
        {
            Content = content,
            DeviceFingerprint = fingerprint,
        }, ct);

    public Task<VoiceResponse> SendVoiceAsync(
        string token,
        byte[] wav,
        string voice,
        string fingerprint,
        CancellationToken ct = default)
    {
        var audio = new ByteArrayContent(wav);
        audio.Headers.ContentType = new MediaTypeHeaderValue("audio/wav");
        var form = new MultipartFormDataContent
        {
            { audio, "audio", "speech.wav" },
            { new StringContent(voice), "voice" },
            { new StringContent(fingerprint), "device_fingerprint" },
        };
        return SendContentAsync<VoiceResponse>(HttpMethod.Post, "/chat/voice", token, form, ct);
    }

    public Task<List<FeatureFlag>> GetFlagsAsync(string token, CancellationToken ct = default) =>
        SendAsync<List<FeatureFlag>>(HttpMethod.Get, "/flags", token, null, ct);

    public Task RegisterDeviceAsync(string token, DeviceHeartbeat body, CancellationToken ct = default) =>
        SendAsync<JsonElement>(HttpMethod.Post, "/me/devices", token, body, ct);

    public Task<ClaimLaunchesResponse> ClaimLaunchesAsync(string token, string fingerprint, CancellationToken ct = default) =>
        SendAsync<ClaimLaunchesResponse>(
            HttpMethod.Post,
            "/me/launches",
            token,
            new ClaimLaunchesRequest { Fingerprint = fingerprint },
            ct);

    private Task<T> SendAsync<T>(HttpMethod method, string path, string? token, object? body, CancellationToken ct)
    {
        HttpContent? content = null;
        if (body is not null)
        {
            content = new StringContent(
                JsonSerializer.Serialize(body, Json),
                Encoding.UTF8,
                "application/json");
        }

        return SendContentAsync<T>(method, path, token, content, ct);
    }

    private async Task<T> SendContentAsync<T>(
        HttpMethod method,
        string path,
        string? token,
        HttpContent? content,
        CancellationToken ct)
    {
        using var request = new HttpRequestMessage(method, BaseUrl.TrimEnd('/') + path);
        request.Headers.Accept.ParseAdd("application/json");
        if (!string.IsNullOrEmpty(token))
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        request.Content = content;

        HttpResponseMessage response;
        try
        {
            response = await _http.SendAsync(request, ct).ConfigureAwait(false);
        }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested)
        {
            throw new ApiException(0, "API nie odpowiedziało w wyznaczonym czasie.");
        }
        catch (HttpRequestException)
        {
            throw new ApiException(0, "Nie można połączyć z API. Sprawdź adres i czy backend działa.");
        }

        var text = await response.Content.ReadAsStringAsync(ct).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
        {
            ApiErrorBody? parsed = null;
            if (!string.IsNullOrWhiteSpace(text))
            {
                try
                {
                    parsed = JsonSerializer.Deserialize<ApiErrorBody>(text, Json);
                }
                catch (JsonException)
                {
                    parsed = null;
                }
            }

            throw new ApiException((int)response.StatusCode, parsed);
        }

        if (string.IsNullOrWhiteSpace(text))
            throw new ApiException((int)response.StatusCode, "Pusta odpowiedź API.");

        var result = JsonSerializer.Deserialize<T>(text, Json);
        if (result is null)
            throw new ApiException((int)response.StatusCode, "Pusta odpowiedź API.");
        return result;
    }
}
