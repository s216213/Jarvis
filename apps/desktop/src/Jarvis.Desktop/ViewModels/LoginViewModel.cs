using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public partial class LoginViewModel : ObservableObject
{
    private readonly SessionController _session;

    public LoginViewModel(SessionController session)
    {
        _session = session;
        _apiBase = session.Settings.ApiBase;
    }

    [ObservableProperty]
    private string _apiBase;

    [ObservableProperty]
    private string _email = "";

    [ObservableProperty]
    private string _password = "";

    [ObservableProperty]
    private string _error = "";

    [ObservableProperty]
    private bool _isBusy;

    public bool HasError => !string.IsNullOrWhiteSpace(Error);
    public string SubmitLabel => IsBusy ? "Logowanie…" : "Zaloguj";

    partial void OnErrorChanged(string value) => OnPropertyChanged(nameof(HasError));

    partial void OnIsBusyChanged(bool value)
    {
        OnPropertyChanged(nameof(SubmitLabel));
        LoginCommand.NotifyCanExecuteChanged();
    }

    partial void OnEmailChanged(string value) => LoginCommand.NotifyCanExecuteChanged();
    partial void OnPasswordChanged(string value) => LoginCommand.NotifyCanExecuteChanged();
    partial void OnApiBaseChanged(string value) => LoginCommand.NotifyCanExecuteChanged();

    private bool CanLogin() =>
        !IsBusy
        && !string.IsNullOrWhiteSpace(Email)
        && !string.IsNullOrWhiteSpace(Password)
        && !string.IsNullOrWhiteSpace(ApiBase);

    [RelayCommand(CanExecute = nameof(CanLogin))]
    private async Task LoginAsync()
    {
        Error = "";
        if (!ApiClient.TryNormalize(ApiBase, out var baseUrl, out var problem))
        {
            Error = problem;
            return;
        }

        IsBusy = true;
        try
        {
            _session.Api.UseBase(baseUrl);
            _session.Settings.ApiBase = baseUrl;
            _session.Settings.Save();
            var result = await _session.Api.LoginAsync(Email.Trim(), Password);
            _session.Settings.Token = result.AccessToken;
            _session.Settings.User = result.User;
            _session.Settings.Save();
            Password = "";
            _session.ShowShell();
        }
        catch (ApiException ex)
        {
            Error = ex.ForLogin();
        }
        catch (Exception ex)
        {
            Error = ex.Message;
        }
        finally
        {
            IsBusy = false;
        }
    }
}
