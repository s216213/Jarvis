using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Jarvis.Desktop.ViewModels;

namespace Jarvis.Desktop;

public partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            var session = new SessionController();
            desktop.MainWindow = new MainWindow
            {
                DataContext = session,
            };
            _ = session.StartAsync();
        }

        base.OnFrameworkInitializationCompleted();
    }
}
