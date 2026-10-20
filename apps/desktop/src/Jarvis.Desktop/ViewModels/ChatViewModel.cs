using System.Collections.ObjectModel;
using System.Collections.Specialized;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jarvis.Desktop.Models;
using Jarvis.Desktop.Services;

namespace Jarvis.Desktop.ViewModels;

public partial class ChatViewModel : ObservableObject
{
    private readonly SessionController _session;

    public ChatViewModel(SessionController session)
    {
        _session = session;
        Messages.CollectionChanged += OnMessagesChanged;
    }

    public ObservableCollection<ChatMessage> Messages { get; } = [];

    [ObservableProperty]
    private string _draft = "";

    [ObservableProperty]
    private string _error = "";

    [ObservableProperty]
    private string _notice = "";

    [ObservableProperty]
    private string _usageText = "";

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private bool _isSending;

    [ObservableProperty]
    private bool _chatEnabled = true;

    public bool IsEmpty => Messages.Count == 0;
    public bool HasError => !string.IsNullOrWhiteSpace(Error);
    public bool HasNotice => !string.IsNullOrWhiteSpace(Notice);
    public bool HasUsage => !string.IsNullOrWhiteSpace(UsageText);
    public string SendLabel => IsSending ? "Myślę…" : "Wyślij";

    partial void OnErrorChanged(string value) => OnPropertyChanged(nameof(HasError));
    partial void OnNoticeChanged(string value) => OnPropertyChanged(nameof(HasNotice));
    partial void OnUsageTextChanged(string value) => OnPropertyChanged(nameof(HasUsage));
    partial void OnDraftChanged(string value) => SendCommand.NotifyCanExecuteChanged();

    partial void OnIsLoadingChanged(bool value) => SendCommand.NotifyCanExecuteChanged();

    partial void OnIsSendingChanged(bool value)
    {
        OnPropertyChanged(nameof(SendLabel));
        SendCommand.NotifyCanExecuteChanged();
    }

    partial void OnChatEnabledChanged(bool value) => SendCommand.NotifyCanExecuteChanged();

    public async Task LoadAsync()
    {
        var token = _session.Settings.Token;
        if (string.IsNullOrEmpty(token))
            return;

        await OnUiAsync(() =>
        {
            IsLoading = true;
            Error = "";
        });
        try
        {
            var messages = await _session.Api.GetMessagesAsync(token).ConfigureAwait(false);
            string usageText;
            try
            {
                usageText = (await _session.Api.GetUsageAsync(token).ConfigureAwait(false)).Format();
            }
            catch (ApiException)
            {
                usageText = "";
            }

            var chatEnabled = true;
            var notice = "";
            try
            {
                var flags = await _session.Api.GetFlagsAsync(token).ConfigureAwait(false);
                var chat = flags.FirstOrDefault(flag => flag.Key == "chat_enabled");
                chatEnabled = chat is null || chat.Enabled;
                notice = chatEnabled ? "" : "Czat jest wyłączony.";
            }
            catch (ApiException)
            {
                chatEnabled = true;
            }

            await OnUiAsync(() =>
            {
                Messages.Clear();
                foreach (var message in messages)
                    Messages.Add(message);
                UsageText = usageText;
                ChatEnabled = chatEnabled;
                Notice = notice;
            });
        }
        catch (ApiException ex)
        {
            var message = ex.ForChat();
            await OnUiAsync(() => Error = message);
        }
        catch (Exception ex)
        {
            var message = ex.Message;
            await OnUiAsync(() => Error = message);
        }
        finally
        {
            await OnUiAsync(() => IsLoading = false);
        }
    }

    private bool CanSend() =>
        !IsLoading
        && !IsSending
        && ChatEnabled
        && !string.IsNullOrWhiteSpace(Draft);

    [RelayCommand(CanExecute = nameof(CanSend))]
    private async Task SendAsync()
    {
        var token = _session.Settings.Token;
        var content = Draft.Trim();
        if (string.IsNullOrEmpty(token) || content.Length == 0 || !ChatEnabled)
            return;

        var knownIds = Messages.Select(message => message.Id).ToHashSet();
        var optimistic = new ChatMessage
        {
            Id = Guid.NewGuid(),
            Role = "user",
            Content = content,
        };

        Draft = "";
        Error = "";
        IsSending = true;
        Messages.Add(optimistic);

        using var cts = new CancellationTokenSource(TimeSpan.FromMinutes(3));
        var postTask = SendAndApplyAsync(token, content, optimistic.Id, cts.Token);
        var pollTask = PollUntilReplyAsync(token, content, optimistic.Id, knownIds, cts.Token);
        try
        {
            var finished = await Task.WhenAny(postTask, pollTask).ConfigureAwait(false);
            if (finished == pollTask && pollTask.IsCompletedSuccessfully && await pollTask.ConfigureAwait(false))
                _ = Observe(postTask);
            else
                await postTask.ConfigureAwait(false);
        }
        catch (ApiException ex)
        {
            var message = ex.ForChat();
            await OnUiAsync(() => Error = message);
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            var message = ex.Message;
            await OnUiAsync(() => Error = message);
        }
        finally
        {
            await cts.CancelAsync();
            await OnUiAsync(() => IsSending = false);
        }
    }

    private async Task SendAndApplyAsync(string token, string content, Guid optimisticId, CancellationToken ct)
    {
        var result = await _session.Api.SendMessageAsync(token, content, ct).ConfigureAwait(false);
        string? usageText = null;
        try
        {
            usageText = (await _session.Api.GetUsageAsync(token, ct).ConfigureAwait(false)).Format();
        }
        catch (ApiException)
        {
            // The reply is already saved. Usage can refresh on the next send.
        }

        await OnUiAsync(() =>
        {
            RemoveMessage(optimisticId);
            AddIfPresent(result.UserMessage);
            AddIfPresent(result.AssistantMessage);
            if (usageText is not null)
                UsageText = usageText;
        });
    }

    private async Task<bool> PollUntilReplyAsync(
        string token,
        string content,
        Guid optimisticId,
        HashSet<Guid> knownIds,
        CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await Task.Delay(700, ct).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return false;
            }

            List<ChatMessage> server;
            try
            {
                server = await _session.Api.GetMessagesAsync(token, ct).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return false;
            }
            catch (ApiException)
            {
                continue;
            }

            var synced = false;
            await OnUiAsync(() => synced = ApplyServerSnapshot(server, content, optimisticId, knownIds));
            if (synced)
                return true;
        }

        return false;
    }

    private bool ApplyServerSnapshot(
        IReadOnlyList<ChatMessage> server,
        string content,
        Guid optimisticId,
        HashSet<Guid> knownIds)
    {
        var fresh = server.Where(message => !knownIds.Contains(message.Id)).ToList();
        if (fresh.Count == 0)
            return false;

        var userSaved = fresh.Any(message => message.Role == "user" && message.Content == content);
        var assistantSaved = userSaved && fresh.Any(message => message.Role == "assistant");

        Messages.Clear();
        foreach (var message in server)
            Messages.Add(message);
        if (!userSaved)
        {
            Messages.Add(new ChatMessage
            {
                Id = optimisticId,
                Role = "user",
                Content = content,
            });
        }

        return assistantSaved;
    }

    private void RemoveMessage(Guid id)
    {
        var existing = Messages.FirstOrDefault(message => message.Id == id);
        if (existing is not null)
            Messages.Remove(existing);
    }

    private void AddIfPresent(ChatMessage? message)
    {
        if (message is null || message.Id == Guid.Empty || Messages.Any(existing => existing.Id == message.Id))
            return;
        Messages.Add(message);
    }

    private static async Task Observe(Task task)
    {
        try
        {
            await task.ConfigureAwait(false);
        }
        catch
        {
            // The reply is already on screen. A late POST failure does not roll the transcript back.
        }
    }

    private static Task OnUiAsync(Action action)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            action();
            return Task.CompletedTask;
        }

        return Dispatcher.UIThread.InvokeAsync(action).GetTask();
    }

    private void OnMessagesChanged(object? sender, NotifyCollectionChangedEventArgs e) =>
        OnPropertyChanged(nameof(IsEmpty));
}
