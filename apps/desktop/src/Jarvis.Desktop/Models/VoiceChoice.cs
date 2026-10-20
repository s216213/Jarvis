namespace Jarvis.Desktop.Models;

public sealed class VoiceChoice
{
    public VoiceChoice(string id, string label)
    {
        Id = id;
        Label = label;
    }

    public string Id { get; }
    public string Label { get; }

    public static IReadOnlyList<VoiceChoice> All { get; } =
    [
        new("bm_george", "George — brytyjski"),
        new("bm_daniel", "Daniel — brytyjski"),
        new("bm_fable", "Fable — brytyjski"),
        new("am_adam", "Adam — amerykański"),
        new("am_michael", "Michael — amerykański"),
    ];

    public static VoiceChoice Find(string? id) =>
        All.FirstOrDefault(choice => choice.Id == id) ?? All[0];
}
