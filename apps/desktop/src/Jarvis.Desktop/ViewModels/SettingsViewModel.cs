using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jarvis.Desktop.Models;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public partial class SettingsViewModel : ObservableObject
{
    private readonly SessionController _session;
    private readonly VoiceViewModel _voice;
    private bool _ready;

    public SettingsViewModel(SessionController session, VoiceViewModel voice)
    {
        _session = session;
        _voice = voice;
        _selectedVoice = VoiceChoice.Find(session.Settings.KokoroVoice);
        if (session.Settings.KokoroVoice != _selectedVoice.Id)
        {
            session.Settings.KokoroVoice = _selectedVoice.Id;
            session.Settings.Save();
        }

        _ready = true;
    }

    public IReadOnlyList<VoiceChoice> Voices { get; } = VoiceChoice.All;

    public string WakeStatus { get; } = WakeListener.StatusText();

    public string Blurb { get; } =
        "W trybie słowa mikrofon zostaje otwarty lokalnie i czeka na „Jarvis”. Do API idzie dopiero komenda. Słuch i synteza stoją na serwerze. W tym trybie Jarvis mówi po angielsku.";

    [ObservableProperty]
    private VoiceChoice _selectedVoice;

    public bool IsPushMode => !string.Equals(_session.Settings.VoiceListenMode, "wake", StringComparison.Ordinal);
    public bool IsWakeMode => !IsPushMode;

    partial void OnSelectedVoiceChanged(VoiceChoice value)
    {
        if (!_ready || value.Id == _session.Settings.KokoroVoice)
            return;
        _session.Settings.KokoroVoice = value.Id;
        _session.Settings.Save();
    }

    [RelayCommand]
    private void UsePush()
    {
        SetMode("push");
    }

    [RelayCommand]
    private void UseWake()
    {
        SetMode("wake");
    }

    private void SetMode(string mode)
    {
        if (string.Equals(_session.Settings.VoiceListenMode, mode, StringComparison.Ordinal))
            return;
        _session.Settings.VoiceListenMode = mode;
        _session.Settings.Save();
        OnPropertyChanged(nameof(IsPushMode));
        OnPropertyChanged(nameof(IsWakeMode));
        _voice.ApplyListenMode();
    }
}
