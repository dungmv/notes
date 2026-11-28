using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Networking.Connectivity;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Notes;

public sealed partial class MainPage
{
    private readonly SemaphoreSlim syncGate = new(1, 1);
    private OfflineSession? syncWorker;
    private SyncStatusDto? syncStatus;
    private DispatcherQueueTimer? pollTimer;
    private DispatcherQueueTimer? postCommitSyncTimer;
    private bool windowActive = true;
    private bool syncInFlight;
    private bool syncQueued;
    private bool pendingSyncReload;
    private NetworkStatusChangedEventHandler? networkHandler;

    private async void InitializeSync(string root)
    {
        try
        {
            syncWorker = await Task.Run(async () =>
                OfflineSession.Open(root, await LocalDbKeyStore.LoadOrCreateAsync(root)));
            var secrets = await SecureKeyStore.LoadAsync(root);
            if (secrets is not null)
            {
                syncStatus = await CallSyncAsync<SyncStatusDto>(new
                {
                    type = "sync_unlock",
                    amk_b64 = secrets.AmkB64,
                    refresh_token = secrets.RefreshToken
                });
            }
            else
            {
                syncStatus = await CallSyncAsync<SyncStatusDto>(new { type = "sync_status" });
            }
        }
        catch (Exception ex)
        {
            SyncStatusText.Text = $"Lỗi khởi tạo đồng bộ: {ex.Message}";
            return;
        }

        pollTimer ??= DispatcherQueue.CreateTimer();
        pollTimer.Interval = TimeSpan.FromSeconds(15);
        pollTimer.IsRepeating = true;
        pollTimer.Tick += PollSyncTick;

        postCommitSyncTimer ??= DispatcherQueue.CreateTimer();
        postCommitSyncTimer.Interval = TimeSpan.FromMilliseconds(1500);
        postCommitSyncTimer.IsRepeating = false;
        postCommitSyncTimer.Tick += PostCommitSyncTick;

        networkHandler = _ => DispatcherQueue.TryEnqueue(() =>
        {
            UpdatePollTimer();
            if (IsNetworkAvailable() && syncStatus is { Enabled: true, Unlocked: true })
                _ = TriggerSyncAsync();
        });
        NetworkInformation.NetworkStatusChanged += networkHandler;

        UpdateSyncUi();
        UpdatePollTimer();
        if (syncStatus is { Enabled: true, Unlocked: true })
            _ = TriggerSyncAsync();
    }

    private void CloseSync()
    {
        if (networkHandler is not null)
        {
            NetworkInformation.NetworkStatusChanged -= networkHandler;
            networkHandler = null;
        }
        if (pollTimer is not null)
        {
            pollTimer.Stop();
            pollTimer.Tick -= PollSyncTick;
            pollTimer = null;
        }
        if (postCommitSyncTimer is not null)
        {
            postCommitSyncTimer.Stop();
            postCommitSyncTimer.Tick -= PostCommitSyncTick;
            postCommitSyncTimer = null;
        }
        var worker = syncWorker;
        syncWorker = null;
        if (worker is not null)
        {
            _ = Task.Run(async () =>
            {
                await syncGate.WaitAsync();
                try { worker.Dispose(); }
                finally { syncGate.Release(); }
            });
        }
    }

    internal void OnWindowActivated(bool isActive)
    {
        windowActive = isActive;
        UpdatePollTimer();
        if (isActive && syncStatus is { Enabled: true, Unlocked: true })
            _ = TriggerSyncAsync();
    }

    private void ScheduleSyncAfterCommit()
    {
        if (syncStatus is not { Enabled: true, Unlocked: true }) return;
        postCommitSyncTimer?.Stop();
        postCommitSyncTimer?.Start();
    }

    private void PollSyncTick(DispatcherQueueTimer timer, object args)
    {
        if (windowActive && IsNetworkAvailable() && syncStatus is { Enabled: true, Unlocked: true })
            _ = TriggerSyncAsync();
    }

    private void PostCommitSyncTick(DispatcherQueueTimer timer, object args)
    {
        if (syncStatus is { Enabled: true, Unlocked: true })
            _ = TriggerSyncAsync();
    }

    private void UpdatePollTimer()
    {
        if (pollTimer is null) return;
        var shouldPoll = windowActive && IsNetworkAvailable() && syncStatus is { Enabled: true, Unlocked: true };
        if (shouldPoll && !pollTimer.IsRunning) pollTimer.Start();
        else if (!shouldPoll && pollTimer.IsRunning) pollTimer.Stop();
    }

    private static bool IsNetworkAvailable()
    {
        try
        {
            var profile = NetworkInformation.GetInternetConnectionProfile();
            return profile is not null &&
                   profile.GetNetworkConnectivityLevel() >= NetworkConnectivityLevel.LocalAccess;
        }
        catch
        {
            return true;
        }
    }

    private void UpdateSyncUi()
    {
        if (syncStatus is null || !syncStatus.Enabled)
        {
            SyncStatusText.Text = "Chưa bật đồng bộ";
            SyncNowButton.IsEnabled = false;
            return;
        }
        SyncNowButton.IsEnabled = syncStatus.Unlocked;
        if (!syncStatus.Unlocked)
        {
            SyncStatusText.Text = "Cần đăng nhập lại";
        }
        else if (!IsNetworkAvailable())
        {
            SyncStatusText.Text = "Chờ mạng";
        }
        else if (syncInFlight)
        {
            SyncStatusText.Text = "Đang đồng bộ…";
        }
        else if (syncStatus.LastError is not null)
        {
            SyncStatusText.Text = $"Lỗi đồng bộ: {syncStatus.LastError}";
        }
        else if (syncStatus.PendingOutboxCount > 0)
        {
            SyncStatusText.Text = $"Chờ gửi ({syncStatus.PendingOutboxCount})";
        }
        else
        {
            SyncStatusText.Text = "Đã đồng bộ";
        }
    }

    private async Task<T> CallSyncAsync<T>(object request)
    {
        var worker = syncWorker ?? throw new InvalidOperationException("Sync worker chưa sẵn sàng.");
        await syncGate.WaitAsync();
        try
        {
            return await Task.Run(() => worker.Call<T>(request));
        }
        finally
        {
            syncGate.Release();
        }
    }

    private async Task CallSyncAsync(object request)
    {
        var worker = syncWorker ?? throw new InvalidOperationException("Sync worker chưa sẵn sàng.");
        await syncGate.WaitAsync();
        try
        {
            await Task.Run(() => worker.Call(request));
        }
        finally
        {
            syncGate.Release();
        }
    }

    private void SyncNowClick(object sender, RoutedEventArgs args) => _ = TriggerSyncAsync();

    private async Task TriggerSyncAsync()
    {
        if (syncWorker is null || syncStatus is not { Enabled: true, Unlocked: true }) return;
        if (!IsNetworkAvailable())
        {
            UpdateSyncUi();
            return;
        }
        if (syncInFlight)
        {
            syncQueued = true;
            return;
        }

        syncInFlight = true;
        UpdateSyncUi();
        try
        {
            var result = await CallSyncAsync<SyncRunResultDto>(new { type = "sync_now" });
            syncStatus = result.Status;
            ApplySyncResultToUi(result);
        }
        catch (SyncUnauthorizedException)
        {
            if (dataRoot is not null) SecureKeyStore.Delete(dataRoot);
            if (syncWorker is not null)
                syncStatus = await CallSyncAsync<SyncStatusDto>(new { type = "sync_clear" });
            UpdatePollTimer();
            Report("Phiên đồng bộ đã hết hạn hoặc thiết bị bị thu hồi. Hãy đăng nhập lại.");
        }
        catch (SyncNetworkException)
        {
            SyncStatusText.Text = "Chờ mạng";
        }
        catch (Exception ex)
        {
            SyncStatusText.Text = $"Lỗi đồng bộ: {ex.Message}";
        }
        finally
        {
            syncInFlight = false;
            UpdateSyncUi();
            if (syncQueued)
            {
                syncQueued = false;
                _ = TriggerSyncAsync();
            }
        }
    }

    private void ApplySyncResultToUi(SyncRunResultDto result)
    {
        if (offline is null) return;
        if (result.FoldersChanged)
            RefreshFolders(currentFolder?.Id ?? "all");
        if (result.NotesListChanged)
            RefreshNotes();

        if (currentNote is not null && result.ChangedNoteIds.Contains(currentNote.Id))
        {
            if (IsComposing)
            {
                pendingSyncReload = true;
                return;
            }
            ReloadCurrentNotePreservingSelection();
        }
    }

    private void ReloadCurrentNotePreservingSelection()
    {
        if (offline is null || currentNote is null) return;
        pendingSyncReload = false;
        var updatedNote = offline.Call<NoteItem>(new { type = "note", id = currentNote.Id });
        if (updatedNote.Deleted != currentNote.Deleted)
        {
            currentNote = null;
            SetCurrent(updatedNote);
            return;
        }

        var (savedBlock, savedCell, savedStart, savedEnd) = active is not null
            ? (active.Id, active.Cell, Selection(active).Item1, Selection(active).Item2)
            : ("", (Cell?)null, 0, 0);

        var snapshot = offline.Call<EditorSnapshot>(new { type = "open_note", id = updatedNote.Id });
        currentNote = updatedNote;
        ShowSnapshot(snapshot);
        if (!string.IsNullOrEmpty(savedBlock) && views.TryGetValue(savedBlock, out var selectedView))
        {
            active = selectedView.Fields.FirstOrDefault(field => field.Cell == savedCell);
            if (active is not null)
            {
                var text = active.Content.Text;
                active.Control.Focus(FocusState.Programmatic);
                active.Control.Document.Selection.SetRange(ClampCaret(text, savedStart), ClampCaret(text, savedEnd));
            }
        }
        SetEditLock();
    }

    private async void SyncAccountClick(object sender, RoutedEventArgs args)
    {
        if (syncWorker is null || dataRoot is null) return;
        await RunDialog(async () =>
        {
            syncStatus = await CallSyncAsync<SyncStatusDto>(new { type = "sync_status" });
            if (syncStatus is { Enabled: true, Unlocked: true })
                await ShowAccountManagementDialog();
            else
                await ShowAuthDialog();
        });
    }

    private async Task ShowAuthDialog()
    {
        var serverBox = new TextBox
        {
            Header = "Máy chủ đồng bộ",
            Text = string.IsNullOrWhiteSpace(syncStatus?.ServerUrl) ? "http://localhost:8080" : syncStatus.ServerUrl
        };
        var emailBox = new TextBox { Header = "Email", Text = syncStatus?.Email ?? "" };
        var passBox = new PasswordBox { Header = "Mật khẩu" };
        var recoveryBox = new TextBox
        {
            Header = "Khóa khôi phục (chỉ dùng khi khôi phục tài khoản)",
            PlaceholderText = "XXXXX-XXXXX-...",
            Visibility = Visibility.Collapsed
        };
        var modeBox = new ComboBox
        {
            Header = "Thao tác",
            ItemsSource = new[] { "Đăng nhập", "Đăng ký mới", "Khôi phục tài khoản" },
            SelectedIndex = 0
        };
        var errorText = new TextBlock { Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.IndianRed), TextWrapping = TextWrapping.Wrap };
        modeBox.SelectionChanged += (_, _) =>
        {
            recoveryBox.Visibility = modeBox.SelectedIndex == 2 ? Visibility.Visible : Visibility.Collapsed;
            passBox.Header = modeBox.SelectedIndex == 2 ? "Mật khẩu mới" : "Mật khẩu";
        };

        var panel = new StackPanel { Spacing = 8, Children = { modeBox, serverBox, emailBox, passBox, recoveryBox, errorText } };
        var dialog = Dialog("Đồng bộ E2EE", panel, "Tiếp tục");
        string? pendingRecoveryKey = null;
        dialog.Closing += async (_, args) =>
        {
            if (args.Result != ContentDialogResult.Primary) return;
            var deferral = args.GetDeferral();
            try
            {
                dialog.IsPrimaryButtonEnabled = false;
                var server = serverBox.Text.Trim();
                var email = emailBox.Text.Trim();
                var pass = passBox.Password;
                if (string.IsNullOrEmpty(server) || string.IsNullOrEmpty(email) || string.IsNullOrEmpty(pass))
                    throw new InvalidOperationException("Vui lòng nhập đầy đủ máy chủ, email và mật khẩu.");

                var deviceName = Environment.MachineName;
                AuthSuccessDto auth = modeBox.SelectedIndex switch
                {
                    1 => await CallSyncAsync<AuthSuccessDto>(new
                    {
                        type = "sync_register",
                        server_url = server,
                        email,
                        passphrase = pass,
                        device_name = deviceName,
                        platform = "windows"
                    }),
                    2 => await CallSyncAsync<AuthSuccessDto>(new
                    {
                        type = "sync_recover",
                        server_url = server,
                        email,
                        recovery_key = recoveryBox.Text.Trim(),
                        new_passphrase = pass,
                        device_name = deviceName,
                        platform = "windows"
                    }),
                    _ => await CallSyncAsync<AuthSuccessDto>(new
                    {
                        type = "sync_login",
                        server_url = server,
                        email,
                        passphrase = pass,
                        device_name = deviceName,
                        platform = "windows"
                    }),
                };

                await SecureKeyStore.SaveAsync(dataRoot!, auth.AmkB64, auth.RefreshToken);
                syncStatus = auth.Status;
                pendingRecoveryKey = auth.RecoveryKey;
                UpdateSyncUi();
                UpdatePollTimer();
            }
            catch (Exception ex)
            {
                args.Cancel = true;
                errorText.Text = ex.Message;
            }
            finally
            {
                dialog.IsPrimaryButtonEnabled = true;
                deferral.Complete();
            }
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
        {
            if (!string.IsNullOrEmpty(pendingRecoveryKey))
                await ShowRecoveryKeyDialog(pendingRecoveryKey);
            _ = TriggerSyncAsync();
        }
    }

    private async Task ShowRecoveryKeyDialog(string recoveryKey)
    {
        var keyBox = new TextBox
        {
            Text = recoveryKey,
            IsReadOnly = true,
            TextWrapping = TextWrapping.Wrap,
            FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("Consolas")
        };
        var confirmCheck = new CheckBox { Content = "Tôi đã lưu khóa khôi phục này ở nơi an toàn" };
        var panel = new StackPanel
        {
            Spacing = 10,
            Children =
            {
                new TextBlock
                {
                    Text = "Khóa khôi phục chỉ hiển thị một lần duy nhất. Bạn cần khóa này nếu quên mật khẩu.",
                    TextWrapping = TextWrapping.Wrap
                },
                keyBox,
                confirmCheck
            }
        };
        var dialog = Dialog("Lưu Khóa Khôi Phục", panel, "Hoàn tất");
        dialog.CloseButtonText = "";
        dialog.IsPrimaryButtonEnabled = false;
        confirmCheck.Checked += (_, _) => dialog.IsPrimaryButtonEnabled = true;
        confirmCheck.Unchecked += (_, _) => dialog.IsPrimaryButtonEnabled = false;
        await dialog.ShowAsync();
    }

    private async Task ShowAccountManagementDialog()
    {
        var info = new TextBlock
        {
            Text = $"Email: {syncStatus!.Email}\nMáy chủ: {syncStatus.ServerUrl}\nDevice ID: {syncStatus.DeviceId}\nCursor: {syncStatus.LastServerCursor} · Chờ gửi: {syncStatus.PendingOutboxCount}",
            TextWrapping = TextWrapping.Wrap
        };
        var changePassBtn = new Button { Content = "Đổi mật khẩu" };
        var devicesBtn = new Button { Content = "Quản lý thiết bị" };
        var logoutBtn = new Button { Content = "Đăng xuất đồng bộ" };

        var panel = new StackPanel
        {
            Spacing = 10,
            Children = { info, new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { changePassBtn, devicesBtn, logoutBtn } } }
        };
        var dialog = Dialog("Tài khoản Đồng bộ", panel, "Đóng");
        dialog.CloseButtonText = "";

        string? nextAction = null;
        changePassBtn.Click += (_, _) => { nextAction = "password"; dialog.Hide(); };
        devicesBtn.Click += (_, _) => { nextAction = "devices"; dialog.Hide(); };
        logoutBtn.Click += (_, _) => { nextAction = "logout"; dialog.Hide(); };

        await dialog.ShowAsync();
        if (nextAction == "password") await ShowChangePasswordDialog();
        else if (nextAction == "devices") await ShowDevicesDialog();
        else if (nextAction == "logout")
        {
            SecureKeyStore.Delete(dataRoot!);
            syncStatus = await CallSyncAsync<SyncStatusDto>(new { type = "sync_clear" });
            UpdateSyncUi();
            UpdatePollTimer();
        }
    }

    private async Task ShowChangePasswordDialog()
    {
        var curPass = new PasswordBox { Header = "Mật khẩu hiện tại" };
        var newPass = new PasswordBox { Header = "Mật khẩu mới" };
        var err = new TextBlock { Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.IndianRed), TextWrapping = TextWrapping.Wrap };
        var dialog = Dialog("Đổi mật khẩu", new StackPanel { Spacing = 8, Children = { curPass, newPass, err } }, "Cập nhật");
        dialog.Closing += async (_, args) =>
        {
            if (args.Result != ContentDialogResult.Primary) return;
            var deferral = args.GetDeferral();
            try
            {
                await CallSyncAsync(new
                {
                    type = "sync_change_password",
                    current_passphrase = curPass.Password,
                    new_passphrase = newPass.Password
                });
                Report("Đã đổi mật khẩu đồng bộ.");
            }
            catch (Exception ex)
            {
                args.Cancel = true;
                err.Text = ex.Message;
            }
            finally { deferral.Complete(); }
        };
        await dialog.ShowAsync();
    }

    private async Task ShowDevicesDialog()
    {
        var devices = await CallSyncAsync<DeviceInfoDto[]>(new { type = "sync_list_devices" });
        var list = new ListView { MaxHeight = 260 };
        list.ItemsSource = devices.Select(d =>
            $"{d.DeviceName} ({d.Platform}) {(d.IsCurrent ? "[Thiết bị này]" : "")} {(d.RevokedAt is not null ? "[Đã thu hồi]" : "")}"
        ).ToArray();
        var revokeBtn = new Button { Content = "Thu hồi thiết bị đã chọn", IsEnabled = false };
        list.SelectionChanged += (_, _) =>
        {
            var idx = list.SelectedIndex;
            revokeBtn.IsEnabled = idx >= 0 && !devices[idx].IsCurrent && devices[idx].RevokedAt is null;
        };

        var dialog = Dialog("Thiết bị đã đăng nhập", new StackPanel { Spacing = 8, Children = { list, revokeBtn } }, "Đóng");
        dialog.CloseButtonText = "";
        revokeBtn.Click += async (_, _) =>
        {
            var idx = list.SelectedIndex;
            if (idx < 0) return;
            await CallSyncAsync(new { type = "sync_revoke_device", target_device_id = devices[idx].Id });
            dialog.Hide();
            Report($"Đã thu hồi thiết bị {devices[idx].DeviceName}.");
        };
        await dialog.ShowAsync();
    }

#if DEBUG
    private async Task VerifyM3(string root)
    {
        var passed = new List<string>();
        string? error = null;
        System.Net.HttpListener? listener = null;
        CancellationTokenSource? serverCts = null;
        try
        {
            while (syncWorker is null) await Task.Delay(25);
            if (SyncStatusText.Text != "Chưa bật đồng bộ" || SyncNowButton.IsEnabled)
                throw new InvalidOperationException($"Trạng thái UI ban đầu sai: {SyncStatusText.Text}");
            passed.Add("Initial WinUI sync status and button state verified");

            var liveServer = Environment.GetEnvironmentVariable("NOTES_M3_LIVE_SERVER");
            string baseUrl;
            var deltasCount = 0;

            if (!string.IsNullOrWhiteSpace(liveServer))
            {
                baseUrl = liveServer.TrimEnd('/');
            }
            else
            {
                // Start local HTTP E2EE sync test server
                var tcp = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
                tcp.Start();
                var port = ((System.Net.IPEndPoint)tcp.LocalEndpoint).Port;
                tcp.Stop();
                baseUrl = $"http://127.0.0.1:{port}";

                listener = new System.Net.HttpListener();
                listener.Prefixes.Add($"{baseUrl}/");
                listener.Start();
                serverCts = new CancellationTokenSource();

                var deltas = new List<System.Text.Json.JsonElement>();
                var userId = Guid.NewGuid().ToString();
                var devicesMap = new Dictionary<string, (string Name, string Platform, long? RevokedAt)>();
                string kdfSalt = "";
                System.Text.Json.JsonElement kdfParams = default;
                string wrappedAmk = "";

                _ = Task.Run(async () =>
                {
                    while (!serverCts.IsCancellationRequested)
                    {
                        System.Net.HttpListenerContext ctx;
                        try { ctx = await listener.GetContextAsync(); }
                        catch { break; }
                        try
                        {
                            using var reader = new StreamReader(ctx.Request.InputStream);
                            var body = await reader.ReadToEndAsync();
                            var path = ctx.Request.Url!.AbsolutePath;
                            object? respObj = null;
                            int status = 200;

                            if (path == "/v1/auth/register")
                            {
                                var doc = System.Text.Json.JsonDocument.Parse(body).RootElement;
                                kdfSalt = doc.GetProperty("kdf_salt_b64").GetString()!;
                                kdfParams = doc.GetProperty("kdf_params").Clone();
                                wrappedAmk = doc.GetProperty("wrapped_amk_b64").GetString()!;
                                var devId = doc.GetProperty("device_id").GetString()!;
                                devicesMap[devId] = (doc.GetProperty("device_name").GetString()!, doc.GetProperty("platform").GetString()!, null);
                                respObj = new
                                {
                                    user_id = userId,
                                    device_id = devId,
                                    access_token = $"tok-{devId}",
                                    refresh_token = $"ref-{devId}",
                                    key_bundle = new { wrapped_amk_b64 = wrappedAmk, key_version = 1 }
                                };
                            }
                            else if (path == "/v1/auth/salt")
                            {
                                respObj = new { kdf_salt_b64 = kdfSalt, kdf_params = kdfParams };
                            }
                            else if (path == "/v1/auth/login")
                            {
                                var doc = System.Text.Json.JsonDocument.Parse(body).RootElement;
                                var devId = doc.GetProperty("device_id").GetString()!;
                                devicesMap[devId] = (doc.GetProperty("device_name").GetString()!, doc.GetProperty("platform").GetString()!, null);
                                respObj = new
                                {
                                    user_id = userId,
                                    device_id = devId,
                                    access_token = $"tok-{devId}",
                                    refresh_token = $"ref-{devId}",
                                    key_bundle = new { wrapped_amk_b64 = wrappedAmk, key_version = 1 }
                                };
                            }
                            else if (path == "/v1/auth/refresh")
                            {
                                var doc = System.Text.Json.JsonDocument.Parse(body).RootElement;
                                var devId = doc.GetProperty("device_id").GetString()!;
                                respObj = new
                                {
                                    user_id = userId,
                                    device_id = devId,
                                    access_token = $"tok2-{devId}",
                                    refresh_token = $"ref2-{devId}",
                                    key_bundle = new { wrapped_amk_b64 = wrappedAmk, key_version = 1 }
                                };
                            }
                            else if (path == "/v1/auth/credentials")
                            {
                                var doc = System.Text.Json.JsonDocument.Parse(body).RootElement;
                                kdfSalt = doc.GetProperty("new_kdf_salt_b64").GetString()!;
                                kdfParams = doc.GetProperty("new_kdf_params").Clone();
                                wrappedAmk = doc.GetProperty("new_wrapped_amk_b64").GetString()!;
                                status = 204;
                            }
                            else if (path == "/v1/devices")
                            {
                                respObj = new
                                {
                                    devices = devicesMap.Select(kv => new
                                    {
                                        id = kv.Key,
                                        device_name = kv.Value.Name,
                                        platform = kv.Value.Platform,
                                        last_seen_at = 1L,
                                        revoked_at = kv.Value.RevokedAt,
                                        created_at = 1L,
                                        is_current = kv.Key == ctx.Request.Headers["X-Device-Id"]
                                    }).ToArray()
                                };
                            }
                            else if (path.StartsWith("/v1/devices/") && ctx.Request.HttpMethod == "DELETE")
                            {
                                var target = path["/v1/devices/".Length..];
                                if (devicesMap.TryGetValue(target, out var d))
                                    devicesMap[target] = (d.Name, d.Platform, 999L);
                                status = 204;
                            }
                            else if (path == "/v1/sync/push")
                            {
                                var devId = ctx.Request.Headers["X-Device-Id"]!;
                                var doc = System.Text.Json.JsonDocument.Parse(body).RootElement;
                                var receipts = new List<object>();
                                foreach (var d in doc.GetProperty("deltas").EnumerateArray())
                                {
                                    var seq = deltas.Count + 1L;
                                    var cloned = System.Text.Json.JsonSerializer.SerializeToElement(new
                                    {
                                        server_seq = seq,
                                        device_id = devId,
                                        entity_type = d.GetProperty("entity_type").GetString(),
                                        entity_id = d.GetProperty("entity_id").GetString(),
                                        client_op_id = d.GetProperty("client_op_id").GetString(),
                                        crypto_version = d.GetProperty("crypto_version").GetInt32(),
                                        key_version = d.GetProperty("key_version").GetInt32(),
                                        encrypted_payload_b64 = d.GetProperty("encrypted_payload_b64").GetString(),
                                        created_at = 1000L
                                    });
                                    deltas.Add(cloned);
                                    deltasCount = deltas.Count;
                                    receipts.Add(new { client_op_id = d.GetProperty("client_op_id").GetString(), server_seq = seq });
                                }
                                respObj = new { receipts };
                            }
                            else if (path == "/v1/sync/pull")
                            {
                                var since = long.Parse(ctx.Request.QueryString["since_seq"] ?? "0");
                                var items = deltas.Where(d => d.GetProperty("server_seq").GetInt64() > since).ToArray();
                                var next = items.Length > 0 ? items[^1].GetProperty("server_seq").GetInt64() : since;
                                respObj = new { deltas = items, next_cursor = next, has_more = false };
                            }

                            ctx.Response.StatusCode = status;
                            if (respObj is not null)
                            {
                                var jsonBytes = System.Text.Encoding.UTF8.GetBytes(System.Text.Json.JsonSerializer.Serialize(respObj));
                                ctx.Response.ContentType = "application/json";
                                await ctx.Response.OutputStream.WriteAsync(jsonBytes);
                            }
                            ctx.Response.Close();
                        }
                        catch { try { ctx.Response.Abort(); } catch { } }
                    }
                });
            }

            // 1. Create offline note on WinUI before enabling sync
            var noteId = offline!.Call<string>(new { type = "create_note", folder = "inbox" });
            RefreshNotes();
            var noteItem = NoteList.Items.OfType<NoteItem>().First(n => n.Id == noteId);
            SetCurrent(noteItem);
            var firstBlockId = views.Values.First().Fields[0].Id;
            Apply(new { type = "replace", id = firstBlockId, start = 0, end = 0, text = "Ghi chú WinUI M3" });

            // 2. Register & DPAPI round-trip
            var email = $"winui-{Guid.NewGuid():N}@example.com";
            var reg = await CallSyncAsync<AuthSuccessDto>(new
            {
                type = "sync_register",
                server_url = baseUrl,
                email,
                passphrase = "m3-password",
                device_name = "WinUI-Host",
                platform = "windows"
            });
            if (string.IsNullOrEmpty(reg.RecoveryKey))
                throw new InvalidOperationException("Không nhận được Recovery Key.");
            await SecureKeyStore.SaveAsync(root, reg.AmkB64, reg.RefreshToken);
            var loaded = await SecureKeyStore.LoadAsync(root);
            if (loaded is null || loaded.AmkB64 != reg.AmkB64 || loaded.RefreshToken != reg.RefreshToken)
                throw new InvalidOperationException("DPAPI SecureKeyStore không khớp.");
            syncStatus = await CallSyncAsync<SyncStatusDto>(new
            {
                type = "sync_unlock",
                amk_b64 = loaded.AmkB64,
                refresh_token = loaded.RefreshToken
            });
            UpdateSyncUi();
            passed.Add("E2EE registration, Recovery Key, and Windows DPAPI unlock verified");

            // 3. Push via TriggerSyncAsync & verify UI status
            await TriggerSyncAsync();
            if (SyncStatusText.Text != "Đã đồng bộ" || !SyncNowButton.IsEnabled || syncStatus!.LastServerCursor < 2)
                throw new InvalidOperationException($"Đồng bộ thất bại: {SyncStatusText.Text}, cursor={syncStatus?.LastServerCursor}, deltas={deltasCount}");
            passed.Add("WinUI bootstrap outbox push and SyncStatusText verified");

            // 4. Simulate second device editing the same note over HTTP and pull into live RichEditBox
            var secondDir = Path.Combine(root, "device2");
            using (var dev2 = await Task.Run(async () =>
                OfflineSession.Open(secondDir, await LocalDbKeyStore.LoadOrCreateAsync(secondDir))))
            {
                dev2.Call<AuthSuccessDto>(new
                {
                    type = "sync_login",
                    server_url = baseUrl,
                    email,
                    passphrase = "m3-password",
                    device_name = "Device-2",
                    platform = "windows"
                });
                dev2.Call<SyncRunResultDto>(new { type = "sync_now" });
                var snap2 = dev2.Call<EditorSnapshot>(new { type = "open_note", id = noteId });
                var blk2 = snap2.Blocks[0].Id;
                dev2.Call<EditorUpdate>(new
                {
                    type = "apply",
                    batch = new
                    {
                        revision = snap2.Revision,
                        operations = new object[]
                        {
                            new { type = "replace", id = blk2, start = 0, end = 0, text = "[Remote] " }
                        },
                        external = false
                    }
                });
                dev2.Call<SyncRunResultDto>(new { type = "sync_now" });
            }

            await TriggerSyncAsync();
            var renderedText = ReadText(views.Values.First().Fields[0]);
            if (renderedText != "[Remote] Ghi chú WinUI M3")
                throw new InvalidOperationException($"Live merge không cập nhật RichEditBox: '{renderedText}'");
            passed.Add("Two-device HTTP E2EE pull and live RichEditBox merge verified");

            // 5. Change password & list/revoke device
            await CallSyncAsync(new
            {
                type = "sync_change_password",
                current_passphrase = "m3-password",
                new_passphrase = "m3-password-2"
            });
            var devs = await CallSyncAsync<DeviceInfoDto[]>(new { type = "sync_list_devices" });
            var otherDev = devs.First(d => !d.IsCurrent);
            await CallSyncAsync(new { type = "sync_revoke_device", target_device_id = otherDev.Id });
            passed.Add("Password change and device revocation verified over HTTP");
        }
        catch (Exception ex)
        {
            error = ex.ToString();
        }
        finally
        {
            serverCts?.Cancel();
            try { listener?.Stop(); listener?.Close(); } catch { }
            var report = System.Text.Json.JsonSerializer.Serialize(new { passed, error });
            File.WriteAllText(Path.Combine(root, "m3-winui-acceptance.json"), report);
            Application.Current.Exit();
        }
    }
#endif
}
