using System.Speech.Recognition;

#pragma warning disable CA1416

namespace Jarvis.Desktop.Services;

public sealed class WakeListener : IDisposable
{
    private SpeechRecognitionEngine? _engine;

    public event Action? Heard;

    public static bool IsAvailable()
    {
        if (!OperatingSystem.IsWindows())
            return false;
        try
        {
            return SpeechRecognitionEngine.InstalledRecognizers()
                .Any(recognizer => recognizer.Culture.Name.StartsWith("en", StringComparison.OrdinalIgnoreCase));
        }
        catch (Exception ex) when (ex is PlatformNotSupportedException or InvalidOperationException)
        {
            return false;
        }
    }

    public void Start()
    {
        Stop();
        var info = SpeechRecognitionEngine.InstalledRecognizers()
            .First(recognizer => recognizer.Culture.Name.StartsWith("en", StringComparison.OrdinalIgnoreCase));
        var engine = new SpeechRecognitionEngine(info);
        var choices = new Choices("Jarvis", "Hey Jarvis", "Hej Jarvis");
        engine.LoadGrammar(new Grammar(new GrammarBuilder(choices) { Culture = info.Culture }));
        engine.SpeechRecognized += (_, args) =>
        {
            if (args.Result is { Confidence: >= 0.4f })
                Heard?.Invoke();
        };
        engine.SetInputToDefaultAudioDevice();
        engine.RecognizeAsync(RecognizeMode.Multiple);
        _engine = engine;
    }

    public void Stop()
    {
        var engine = _engine;
        _engine = null;
        if (engine is null)
            return;
        try
        {
            engine.RecognizeAsyncCancel();
        }
        catch (InvalidOperationException)
        {
            // Already idle.
        }

        try
        {
            engine.SetInputToNull();
        }
        catch (InvalidOperationException)
        {
            // The device is already released.
        }

        engine.Dispose();
    }

    public void Dispose() => Stop();

    public static string StatusText() =>
        IsAvailable()
            ? "Słowo „Jarvis” jest dostępne na tym komputerze."
            : "Brak rozpoznawania mowy en-US. Działa tylko przycisk.";
}
