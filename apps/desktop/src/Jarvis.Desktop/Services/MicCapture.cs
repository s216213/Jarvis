using NAudio.Wave;

namespace Jarvis.Desktop.Services;

public sealed class MicCapture : IDisposable
{
    private WaveInEvent? _wave;
    private MemoryStream _pcm = new();
    private readonly object _gate = new();

    public event Action<double>? LevelChanged;

    public double LatestLevel { get; private set; }

    public void Start()
    {
        StopWave();
        lock (_gate)
            _pcm = new MemoryStream();
        LatestLevel = 0;
        _wave = new WaveInEvent
        {
            WaveFormat = new WaveFormat(16000, 16, 1),
            BufferMilliseconds = 50,
        };
        _wave.DataAvailable += OnData;
        _wave.StartRecording();
    }

    public byte[] StopWav()
    {
        StopWave();
        byte[] pcm;
        lock (_gate)
            pcm = _pcm.ToArray();
        LatestLevel = 0;
        return PcmWav.Wrap(pcm);
    }

    public void Dispose() => StopWave();

    private void StopWave()
    {
        var wave = _wave;
        _wave = null;
        if (wave is null)
            return;
        wave.DataAvailable -= OnData;
        try
        {
            wave.StopRecording();
        }
        catch (Exception ex) when (ex is NAudio.MmException or ObjectDisposedException)
        {
            // The device can already be closed when the turn ends.
        }

        wave.Dispose();
    }

    private void OnData(object? sender, WaveInEventArgs e)
    {
        if (e.BytesRecorded <= 0)
            return;
        lock (_gate)
            _pcm.Write(e.Buffer, 0, e.BytesRecorded);

        double sum = 0;
        var samples = e.BytesRecorded / 2;
        for (var i = 0; i + 1 < e.BytesRecorded; i += 2)
        {
            var sample = BitConverter.ToInt16(e.Buffer, i) / 32768.0;
            sum += sample * sample;
        }

        var rms = samples == 0 ? 0 : Math.Sqrt(sum / samples);
        LatestLevel = Math.Min(1, rms * 4);
        LevelChanged?.Invoke(LatestLevel);
    }
}
