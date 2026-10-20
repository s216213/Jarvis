using CommunityToolkit.Mvvm.ComponentModel;
using Jarvis.Desktop.Models;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public partial class SessionController : ObservableObject
{
    public SessionController()
    {
        Settings = SettingsStore.Load();
        Api = new ApiClient();
        try
        {
            Api.UseBase(Settings.ApiBase);
        }
        catch (InvalidOperationException)
        {
            // The saved address is repaired on the next login.
        }

    }

    public SettingsStore Settings { get; }
    public ApiClient Api { get; }

    [ObservableProperty]
    private object _current = new BootViewModel();

    public async Task StartAsync()
    {
        if (string.IsNullOrWhiteSpace(Settings.Token))
        {
            ShowLogin();
            return;
        }

        try
        {
            Api.UseBase(Settings.ApiBase);
            Settings.User = await Api.GetMeAsync(Settings.Token);
            Settings.Save();
            ShowShell();
        }
        catch (ApiException ex) when (ex.Status is 401 or 403)
        {
            Settings.Token = null;
            Settings.User = null;
            Settings.Save();
            ShowLogin();
        }
        catch
        {
            ShowShell();
        }
    }

    public void ShowLogin(string? message = null)
    {
        Current = new LoginViewModel(this)
        {
            Error = message ?? "",
        };
    }

    public void ShowShell()
    {
        var shell = new ShellViewModel(this);
        Current = shell;
        _ = shell.EnterAsync();
    }

    public async Task RegisterDeviceAsync(string token)
    {
        try
        {
            var (systemName, systemVersion) = HostSystem.Describe();
            await Api.RegisterDeviceAsync(token, new DeviceHeartbeat
            {
                Name = HostSystem.DeviceName(),
                Platform = "desktop",
                UiVersion = AppInfo.UiVersion,
                ClientVersion = AppInfo.ClientVersion,
                SystemName = systemName,
                SystemVersion = systemVersion,
                Fingerprint = Settings.Fingerprint,
            });
        }
        catch
        {
            // A failed heartbeat must not block the chat. The web client does the same.
        }
    }
}
