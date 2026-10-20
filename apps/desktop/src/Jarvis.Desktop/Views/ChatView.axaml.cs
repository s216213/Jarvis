using System.Collections.Specialized;
using Avalonia.Controls;
using Avalonia.Threading;
using Jarvis.Desktop.ViewModels;

namespace Jarvis.Desktop.Views;

public partial class ChatView : UserControl
{
    private ChatViewModel? _hooked;

    public ChatView()
    {
        InitializeComponent();
        DataContextChanged += (_, _) => Hook();
        Hook();
    }

    private void Hook()
    {
        if (_hooked is not null)
            _hooked.Messages.CollectionChanged -= OnMessagesChanged;
        _hooked = DataContext as ChatViewModel;
        if (_hooked is not null)
            _hooked.Messages.CollectionChanged += OnMessagesChanged;
    }

    private void OnMessagesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        Dispatcher.UIThread.Post(() => Log.ScrollToEnd(), DispatcherPriority.Background);
    }
}
