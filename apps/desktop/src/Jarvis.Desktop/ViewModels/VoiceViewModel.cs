using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using Jarvis.Desktop.Models;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public partial class VoiceViewModel : ObservableObject, IDisposable
{
    private readonly SessionController _session;
    private readonly ChatViewModel _chat;
    private readonly MicCapture _mic = new();
    private readonly VoicePlayer _player = new();
    private readonly DispatcherTimer _watch;
    private readonly DispatcherTimer _meter;
    private WakeListener? _wake;
    private DateTime _recordingStarted;
    private DateTime _lastLoud;
    private DateTime _pressAt;
    private bool _heardSpeech;
    private bool _wakeCapture;
    private bool _pressStartedRecording;
    private bool _closing;
    private bool _alive = true;

    public VoiceViewModel(SessionController session, ChatViewModel chat)
    {
        _session = session;
        _chat = chat;
        _mic.LevelChanged += level =>
        {
            if (Phase == "listening")
                Post(() => Level = level);
        };
        _player.Finished += () => Post(OnPlaybackFinished);
        _watch = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(100) };
        _watch.Tick += (_, _) => OnWatch();
        _meter = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(50) };
        _meter.Tick += (_, _) =>
        {
            if (Phase == "speaking")
                Level = _player.ReadLevel();
        };
    }

    public event Action? StageRequested;

    [ObservableProperty]
    private string _phase = "idle";

    [ObservableProperty]
    private double _level;

    [ObservableProperty]
    private string _heard = "";

    [ObservableProperty]
    private string _reply = "";

    [ObservableProperty]
    private string _error = "";

    [ObservableProperty]
    private string _notice = "";

    [ObservableProperty]
    private bool _voiceEnabled = true;

    public bool HasError => !string.IsNullOrWhiteSpace(Error);
    public bool HasNotice => !string.IsNullOrWhiteSpace(Notice);
    public bool HasHeard => !string.IsNullOrWhiteSpace(Heard);
    public bool HasReply => !string.IsNullOrWhiteSpace(Reply);
    public string StatusLabel => Phase switch
    {
        "listening" => "SŁUCHAM",
        "thinking" => "ANALIZA",
        "speaking" => "MÓWIĘ",
        _ => "CISZA",
    };
    public string MicLabel => Phase == "listening" ? "Stop" : "Mów";
    public bool MicEnabled => VoiceEnabled && Phase is "idle" or "listening";

    partial void OnPhaseChanged(string value)
    {
        OnPropertyChanged(nameof(StatusLabel));
        OnPropertyChanged(nameof(MicLabel));
        OnPropertyChanged(nameof(MicEnabled));
    }

    partial void OnErrorChanged(string value) => OnPropertyChanged(nameof(HasError));
    partial void OnNoticeChanged(string value) => OnPropertyChanged(nameof(HasNotice));
    partial void OnHeardChanged(string value) => OnPropertyChanged(nameof(HasHeard));
    partial void OnReplyChanged(string value) => OnPropertyChanged(nameof(HasReply));
    partial void OnVoiceEnabledChanged(bool value) => OnPropertyChanged(nameof(MicEnabled));

    public async Task LoadAsync()
    {
        var token = _session.Settings.Token;
        if (string.IsNullOrEmpty(token))
            return;
        try
        {
            var flags = await _session.Api.GetFlagsAsync(token).ConfigureAwait(false);
            var voice = flags.FirstOrDefault(flag => flag.Key == "voice_enabled");
            var enabled = voice is null || voice.Enabled;
            await OnUiAsync(() =>
            {
                VoiceEnabled = enabled;
                if (!enabled)
                    Notice = "Tryb głosowy jest wyłączony.";
            });
        }
        catch (ApiException)
        {
            await OnUiAsync(() => VoiceEnabled = true);
        }
    }

    public void ApplyListenMode()
    {
        if (Phase != "idle")
            return;
        RestartWake();
    }

    public void OnMicPressed()
    {
        if (!MicEnabled)
            return;
        if (Phase == "listening")
        {
            _pressStartedRecording = false;
            RequestFinish(send: true);
            return;
        }

        _pressAt = DateTime.UtcNow;
        _pressStartedRecording = true;
        StartRecording(wakeFollowUp: false);
    }

    public void OnMicReleased()
    {
        if (!_pressStartedRecording || Phase != "listening")
            return;
        var held = DateTime.UtcNow - _pressAt >= TimeSpan.FromMilliseconds(350);
        _pressStartedRecording = false;
        if (held)
            RequestFinish(send: true);
    }

    public void Shutdown()
    {
        _alive = false;
        _watch.Stop();
        _meter.Stop();
        _wake?.Stop();
        try
        {
            if (Phase == "listening")
                _mic.StopWav();
        }
        catch (Exception ex) when (ex is InvalidOperationException or NAudio.MmException)
        {
            // Logout must not throw if the microphone is already closed.
        }

        _player.Stop();
        Phase = "idle";
        Level = 0;
    }

    public void Dispose()
    {
        Shutdown();
        _mic.Dispose();
        _player.Dispose();
        _wake?.Dispose();
    }

    private void StartRecording(bool wakeFollowUp)
    {
        if (!_alive || !VoiceEnabled || Phase is "thinking" or "speaking")
            return;
        _wake?.Stop();
        _player.Stop();
        _meter.Stop();
        Error = "";
        Notice = "";
        _closing = false;
        _wakeCapture = wakeFollowUp;
        _heardSpeech = false;
        _recordingStarted = DateTime.UtcNow;
        _lastLoud = _recordingStarted;
        try
        {
            _mic.Start();
        }
        catch (Exception ex) when (ex is NAudio.MmException or InvalidOperationException)
        {
            Error = "Nie można otworzyć mikrofonu.";
            RestartWake();
            return;
        }

        Phase = "listening";
        _watch.Start();
    }

    private void OnWatch()
    {
        if (Phase != "listening")
            return;
        var elapsed = DateTime.UtcNow - _recordingStarted;
        if (_mic.LatestLevel > 0.08)
        {
            _heardSpeech = true;
            _lastLoud = DateTime.UtcNow;
        }

        if (elapsed >= TimeSpan.FromSeconds(19.5))
        {
            RequestFinish(send: true);
            return;
        }

        if (!_wakeCapture)
            return;
        if (_heardSpeech && DateTime.UtcNow - _lastLoud >= TimeSpan.FromMilliseconds(1200))
            RequestFinish(send: true);
        else if (!_heardSpeech && elapsed >= TimeSpan.FromSeconds(8))
            RequestFinish(send: false);
    }

    private void RequestFinish(bool send)
    {
        if (_closing || Phase != "listening")
            return;
        _closing = true;
        _ = FinishAsync(send);
    }

    private async Task FinishAsync(bool send)
    {
        _watch.Stop();
        _pressStartedRecording = false;
        byte[] wav;
        try
        {
            wav = _mic.StopWav();
        }
        catch (Exception ex) when (ex is NAudio.MmException or InvalidOperationException)
        {
            Phase = "idle";
            Level = 0;
            Error = "Nie można zamknąć mikrofonu.";
            RestartWake();
            return;
        }

        Level = 0;
        var pcmBytes = Math.Max(0, wav.Length - 44);
        if (!send || pcmBytes < 1600)
        {
            Phase = "idle";
            if (_wakeCapture && !send)
                Notice = "Nie usłyszałem komendy.";
            RestartWake();
            return;
        }

        await SendAsync(wav);
    }

    private async Task SendAsync(byte[] wav)
    {
        var token = _session.Settings.Token;
        if (string.IsNullOrEmpty(token))
        {
            Phase = "idle";
            RestartWake();
            return;
        }

        Phase = "thinking";
        Error = "";
        using var cts = new CancellationTokenSource(TimeSpan.FromMinutes(3));
        try
        {
            var voice = VoiceChoice.Find(_session.Settings.KokoroVoice).Id;
            var result = await _session.Api.SendVoiceAsync(token, wav, voice, cts.Token).ConfigureAwait(false);
            await OnUiAsync(() =>
            {
                if (!_alive)
                    return;
                Heard = result.UserMessage.Content;
                Reply = result.AssistantMessage.Content;
                _chat.AppendSaved(result.UserMessage, result.AssistantMessage);
                Notice = result.SpeechError ?? "";
                if (!string.IsNullOrEmpty(result.AudioBase64))
                {
                    var audio = Convert.FromBase64String(result.AudioBase64);
                    Phase = "speaking";
                    try
                    {
                        _player.Play(audio);
                        _meter.Start();
                    }
                    catch (Exception ex) when (ex is NAudio.MmException or InvalidOperationException)
                    {
                        Phase = "idle";
                        Notice = "Nie można odtworzyć odpowiedzi.";
                        RestartWake();
                    }
                }
                else
                {
                    Phase = "idle";
                    RestartWake();
                }
            });
        }
        catch (ApiException ex)
        {
            var message = ex.ForVoice();
            await OnUiAsync(() =>
            {
                if (!_alive)
                    return;
                Error = message;
                Phase = "idle";
                RestartWake();
            });
        }
        catch (FormatException)
        {
            await OnUiAsync(() =>
            {
                if (!_alive)
                    return;
                Error = "Odpowiedź głosowa jest uszkodzona.";
                Phase = "idle";
                RestartWake();
            });
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            var message = ex.Message;
            await OnUiAsync(() =>
            {
                if (!_alive)
                    return;
                Error = message;
                Phase = "idle";
                RestartWake();
            });
        }
    }

    private void OnPlaybackFinished()
    {
        if (Phase != "speaking")
            return;
        _meter.Stop();
        Level = 0;
        Phase = "idle";
        RestartWake();
    }

    private void OnWake()
    {
        Post(() =>
        {
            if (!_alive || Phase != "idle" || !VoiceEnabled)
                return;
            StageRequested?.Invoke();
            _wake?.Stop();
            StartRecording(wakeFollowUp: true);
        });
    }

    private void RestartWake()
    {
        _wake?.Stop();
        if (!_alive || !VoiceEnabled || Phase != "idle")
            return;
        if (!string.Equals(_session.Settings.VoiceListenMode, "wake", StringComparison.Ordinal))
            return;
        if (!WakeListener.IsAvailable())
            return;
        try
        {
            _wake ??= new WakeListener();
            _wake.Heard -= OnWake;
            _wake.Heard += OnWake;
            _wake.Start();
        }
        catch (Exception ex) when (ex is InvalidOperationException or PlatformNotSupportedException)
        {
            Notice = WakeListener.StatusText();
        }
    }

    private static void Post(Action action)
    {
        if (Dispatcher.UIThread.CheckAccess())
            action();
        else
            Dispatcher.UIThread.Post(action);
    }

    private static Task OnUiAsync(Action action)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            action();
            return Task.CompletedTask;
        }

        return Dispatcher.UIThread.InvokeAsync(action).GetTask();
    }
}
