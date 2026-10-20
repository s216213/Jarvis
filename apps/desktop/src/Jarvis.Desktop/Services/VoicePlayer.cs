using NAudio.Wave;

namespace Jarvis.Desktop.Services;

public sealed class VoicePlayer : IDisposable
{
    private WaveOutEvent? _output;
    private WaveFileReader? _reader;
    private short[] _samples = [];
    private int _channels = 1;
    private bool _expectFinish;

    public event Action? Finished;

    public void Play(byte[] wav)
    {
        Stop(raiseFinished: false);
        _samples = ReadPcm16(wav, out _channels);
        var stream = new MemoryStream(wav, writable: false);
        _reader = new WaveFileReader(stream);
        _output = new WaveOutEvent();
        _output.Init(_reader);
        _output.PlaybackStopped += OnStopped;
        _expectFinish = true;
        _output.Play();
    }

    public double ReadLevel()
    {
        var output = _output;
        var reader = _reader;
        if (output is null || reader is null || _samples.Length == 0)
            return 0;
        var frame = Math.Max(1, reader.WaveFormat.BlockAlign);
        var index = (int)(output.GetPosition() / frame) * Math.Max(1, _channels);
        if (index < 0 || index >= _samples.Length)
            return 0;
        var take = Math.Min(_samples.Length - index, reader.WaveFormat.SampleRate / 20);
        double sum = 0;
        for (var i = 0; i < take; i++)
        {
            var sample = _samples[index + i] / 32768.0;
            sum += sample * sample;
        }

        return Math.Min(1, Math.Sqrt(sum / take) * 4);
    }

    public void Stop(bool raiseFinished = false)
    {
        _expectFinish = raiseFinished;
        var output = _output;
        _output = null;
        if (output is not null)
        {
            output.PlaybackStopped -= OnStopped;
            try
            {
                output.Stop();
            }
            catch (Exception ex) when (ex is NAudio.MmException or ObjectDisposedException)
            {
                // Already stopped.
            }

            output.Dispose();
        }

        _reader?.Dispose();
        _reader = null;
        _samples = [];
        if (raiseFinished)
            Finished?.Invoke();
    }

    public void Dispose() => Stop();

    private void OnStopped(object? sender, StoppedEventArgs e)
    {
        if (!_expectFinish)
            return;
        _expectFinish = false;
        Finished?.Invoke();
    }

    private static short[] ReadPcm16(byte[] wav, out int channels)
    {
        channels = 1;
        if (wav.Length < 44 || wav[0] != 'R' || wav[8] != 'W')
            return [];
        var offset = 12;
        var bits = 16;
        var dataStart = -1;
        var dataLength = 0;
        while (offset + 8 <= wav.Length)
        {
            var size = BitConverter.ToInt32(wav, offset + 4);
            if (size < 0 || offset + 8 + size > wav.Length)
                break;
            var id = System.Text.Encoding.ASCII.GetString(wav, offset, 4);
            if (id == "fmt " && size >= 16)
            {
                channels = BitConverter.ToInt16(wav, offset + 8 + 2);
                bits = BitConverter.ToInt16(wav, offset + 8 + 14);
            }
            else if (id == "data")
            {
                dataStart = offset + 8;
                dataLength = size;
            }

            offset += 8 + size + (size % 2);
        }

        if (bits != 16 || dataStart < 0 || channels < 1)
        {
            channels = 1;
            return [];
        }

        var count = dataLength / 2;
        var samples = new short[count];
        Buffer.BlockCopy(wav, dataStart, samples, 0, count * 2);
        return samples;
    }
}
