using System.Globalization;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public enum ShellPage
{
    Chat,
    Voice,
    Settings,
}

public partial class ShellViewModel : ObservableObject
{
    private readonly SessionController _session;
    private readonly DispatcherTimer _clock;

    public ShellViewModel(SessionController session)
    {
        _session = session;
        Chat = new ChatViewModel(session);
        Voice = new VoiceViewModel(session, Chat);
        SettingsPage = new SettingsViewModel(session, Voice);
        Voice.StageRequested += () => Page = ShellPage.Voice;
        _clock = new DispatcherTimer { Interval = TimeSpan.FromSeconds(30) };
        _clock.Tick += (_, _) => UpdateClock();
        UpdateClock();
        _clock.Start();
    }

    public ChatViewModel Chat { get; }
    public VoiceViewModel Voice { get; }
    public SettingsViewModel SettingsPage { get; }

    [ObservableProperty]
    private ShellPage _page = ShellPage.Chat;

    public bool IsChat => Page == ShellPage.Chat;
    public bool IsVoice => Page == ShellPage.Voice;
    public bool IsSettings => Page == ShellPage.Settings;

    partial void OnPageChanged(ShellPage value)
    {
        OnPropertyChanged(nameof(IsChat));
        OnPropertyChanged(nameof(IsVoice));
        OnPropertyChanged(nameof(IsSettings));
    }
    public string UiLabel { get; } = AppInfo.UiVersion.ToUpperInvariant();
    public string ClientVersion { get; } = AppInfo.ClientVersion;
    public string DisplayName => _session.Settings.User?.DisplayName ?? "";

    [ObservableProperty]
    private string _greeting = "Witaj";

    [ObservableProperty]
    private string _clockText = "";

    [ObservableProperty]
    private string _brainVersion = "";

    [ObservableProperty]
    private string _hostSystem = "—";

    public string Headline =>
        string.IsNullOrWhiteSpace(DisplayName) ? Greeting : $"{Greeting}, {DisplayName}";

    public bool HasBrain => !string.IsNullOrWhiteSpace(BrainVersion);

    partial void OnGreetingChanged(string value) => OnPropertyChanged(nameof(Headline));
    partial void OnBrainVersionChanged(string value) => OnPropertyChanged(nameof(HasBrain));

    public async Task EnterAsync()
    {
        var token = _session.Settings.Token;
        if (string.IsNullOrEmpty(token))
            return;

        var versionTask = LoadVersionAsync(token);
        var deviceTask = _session.RegisterDeviceAsync(token);
        var chatTask = Chat.LoadAsync();
        var voiceTask = Voice.LoadAsync();
        await Task.WhenAll(versionTask, deviceTask, chatTask, voiceTask);
        Voice.ApplyListenMode();
    }

    [RelayCommand]
    private void ShowChat() => Page = ShellPage.Chat;

    [RelayCommand]
    private void ShowVoice() => Page = ShellPage.Voice;

    [RelayCommand]
    private void ShowSettings() => Page = ShellPage.Settings;

    [RelayCommand]
    private void Logout()
    {
        Voice.Shutdown();
        _clock.Stop();
        _session.Settings.Token = null;
        _session.Settings.User = null;
        _session.Settings.Save();
        _session.ShowLogin();
    }

    private async Task LoadVersionAsync(string token)
    {
        try
        {
            var version = await _session.Api.GetVersionAsync(token);
            BrainVersion = version.BrainVersion;
            HostSystem = version.HostLabel();
        }
        catch (ApiException)
        {
            BrainVersion = "";
            HostSystem = "—";
        }
    }

    private void UpdateClock()
    {
        var now = DateTime.Now;
        Greeting = now.Hour < 18 ? "Dzień dobry" : "Dobry wieczór";
        ClockText = now.ToString("ddd d MMM, HH:mm", CultureInfo.GetCultureInfo("pl-PL"));
    }
}
