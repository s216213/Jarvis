namespace Jarvis.Desktop.Services;

public static class PcmWav
{
    public static byte[] Wrap(byte[] pcm, int sampleRate = 16000)
    {
        var dataLength = pcm.Length;
        var bytes = new byte[44 + dataLength];
        bytes[0] = (byte)'R';
        bytes[1] = (byte)'I';
        bytes[2] = (byte)'F';
        bytes[3] = (byte)'F';
        WriteInt(bytes, 4, 36 + dataLength);
        bytes[8] = (byte)'W';
        bytes[9] = (byte)'A';
        bytes[10] = (byte)'V';
        bytes[11] = (byte)'E';
        bytes[12] = (byte)'f';
        bytes[13] = (byte)'m';
        bytes[14] = (byte)'t';
        bytes[15] = (byte)' ';
        WriteInt(bytes, 16, 16);
        WriteShort(bytes, 20, 1);
        WriteShort(bytes, 22, 1);
        WriteInt(bytes, 24, sampleRate);
        WriteInt(bytes, 28, sampleRate * 2);
        WriteShort(bytes, 32, 2);
        WriteShort(bytes, 34, 16);
        bytes[36] = (byte)'d';
        bytes[37] = (byte)'a';
        bytes[38] = (byte)'t';
        bytes[39] = (byte)'a';
        WriteInt(bytes, 40, dataLength);
        Buffer.BlockCopy(pcm, 0, bytes, 44, dataLength);
        return bytes;
    }

    private static void WriteInt(byte[] bytes, int offset, int value)
    {
        bytes[offset] = (byte)value;
        bytes[offset + 1] = (byte)(value >> 8);
        bytes[offset + 2] = (byte)(value >> 16);
        bytes[offset + 3] = (byte)(value >> 24);
    }

    private static void WriteShort(byte[] bytes, int offset, short value)
    {
        bytes[offset] = (byte)value;
        bytes[offset + 1] = (byte)(value >> 8);
    }
}
