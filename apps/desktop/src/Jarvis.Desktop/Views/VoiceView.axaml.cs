using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Jarvis.Desktop.ViewModels;

namespace Jarvis.Desktop.Views;

public partial class VoiceView : UserControl
{
    public VoiceView()
    {
        InitializeComponent();
        // The button marks the pointer event handled, so a normal XAML handler never runs.
        MicButton.AddHandler(PointerPressedEvent, OnMicPressed, RoutingStrategies.Tunnel, handledEventsToo: true);
        MicButton.AddHandler(PointerReleasedEvent, OnMicReleased, RoutingStrategies.Tunnel, handledEventsToo: true);
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
