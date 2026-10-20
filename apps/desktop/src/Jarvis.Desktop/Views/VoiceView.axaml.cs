using Avalonia.Controls;
using Avalonia.Input;
using Jarvis.Desktop.ViewModels;

namespace Jarvis.Desktop.Views;

public partial class VoiceView : UserControl
{
    public VoiceView()
    {
        InitializeComponent();
    }

    private void OnMicPressed(object? sender, PointerPressedEventArgs e)
    {
        if (DataContext is not VoiceViewModel voice)
            return;
        e.Pointer.Capture(MicButton);
        voice.OnMicPressed();
    }

    private void OnMicReleased(object? sender, PointerReleasedEventArgs e)
    {
        if (e.Pointer.Captured == MicButton)
            e.Pointer.Capture(null);
        if (DataContext is VoiceViewModel voice)
            voice.OnMicReleased();
    }
}
