using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;
using Windows.System;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Notes;

public sealed partial class MainPage
{
    internal static string? DataRootStatic;

    private OfflineSession? offline;
    private readonly bool prototypeMode = Environment.GetCommandLineArgs().Contains("--m0-replay") ||
        Environment.GetEnvironmentVariable("NOTES_M0_REPLAY") == "1";
    private NoteItem? currentNote;
    private FolderItem? currentFolder;
    private FolderItem[] folders = [];
    private readonly Dictionary<string, (string Block, Cell? Cell, int Start, int End)> savedSelections = [];
    private DispatcherQueueTimer? refreshTimer;
    private bool refreshing;
    private bool opening;
    private CancellationTokenSource? lifetime;
    private bool dialogOpen;
    private bool loadingImage;
    private string? dataRoot;
    private (Func<EditorUpdate> Call, bool Redo)? pending;
    private (string Id, Cell? Cell, int Start, int End, string Query)? lastMatch;
    private bool lockSupported;
    private string? lockedSessionNoteId;
    private DispatcherQueueTimer? autoLockTimer;
    private const int AutoLockMinutes = 5;

    private bool ReadOnlyNote => !prototypeMode && (currentNote?.Deleted != false || pending is not null);
    private bool IsComposing => views.Values.SelectMany(view => view.Fields).Any(item => item.Composing);

    private async void InitializeEditor()
    {
        if (prototypeMode)
        {
            FolderColumn.Width = new GridLength(0);
            NoteColumn.Width = new GridLength(0);
            FolderPane.Visibility = NotePane.Visibility = NoteActions.Visibility = Visibility.Collapsed;
            PrototypeActions.Visibility = Visibility.Visible;
            Run(Reset);
            return;
        }
        if (offline is not null || opening) return;
        using var cancellation = new CancellationTokenSource();
        lifetime = cancellation;
        opening = true;
        try { await OpenOffline(cancellation.Token); }
        catch (Exception error) when (!cancellation.IsCancellationRequested) { Report(error.Message); }
        catch (Exception) when (cancellation.IsCancellationRequested) { }
        finally
        {
            opening = false;
            lifetime = null;
        }
    }

    private async Task OpenOffline(CancellationToken cancellation)
    {
        var args = Environment.GetCommandLineArgs();
        var index = Array.IndexOf(args, "--data-dir");
        if (index == args.Length - 1) throw new ArgumentException("--data-dir cần đường dẫn.");
        var root = (index >= 0 ? args[index + 1] : Environment.GetEnvironmentVariable("NOTES_DATA_DIR"))
            ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Notes");
        root = Path.GetFullPath(root);
#if DEBUG
        var verify = args.Contains("--verify-m1");
        var verifyM3 = args.Contains("--verify-m3");
        var verifyM4 = args.Contains("--verify-m4");
        var m2 = args.SingleOrDefault(arg => arg.StartsWith("--m2-", StringComparison.Ordinal));
        if ((verify || verifyM3 || verifyM4) && (index < 0 || File.Exists(Path.Combine(root, "notes.sqlite3"))))
            throw new InvalidOperationException("--verify cần --data-dir trỏ tới thư mục thử nghiệm mới.");
        if (m2 is not null && (index < 0 || !File.Exists(Path.Combine(root, "notes.sqlite3"))))
            throw new InvalidOperationException("Nghiệm thu M2 cần --data-dir chứa fixture desktop.");
#endif
        var session = await Task.Run(async () =>
            OfflineSession.Open(root, await LocalDbKeyStore.LoadOrCreateAsync(root)), cancellation);
        if (cancellation.IsCancellationRequested)
        {
            session.Dispose();
            return;
        }
        offline = session;
        dataRoot = root;
        DataRootStatic = root;
        ChecklistSort.IsChecked = offline.Call<bool>(new { type = "checklist_sort" });
        lockSupported = offline.Call<LockStatusDto>(new { type = "lock_status" }).Supported;
        refreshTimer ??= DispatcherQueue.CreateTimer();
        refreshTimer.Interval = TimeSpan.FromMilliseconds(180);
        refreshTimer.IsRepeating = false;
        refreshTimer.Tick += RefreshTick;
        SetupAutoLock();
        RefreshFolders("all");
        RefreshNotes();
        SetCurrent(null);
        InitializeSync(root);
        GlobalSearch.Focus(FocusState.Programmatic);
#if DEBUG
        if (verify) File.AppendAllText(Path.Combine(root, "verify-trace.log"),
            $"{DateTime.Now:HH:mm:ss.fff} open-offline-done\n");
        if (verify) DispatcherQueue.TryEnqueue(() => _ = VerifyOffline(root));
        if (verifyM3) DispatcherQueue.TryEnqueue(() => _ = VerifyM3(root));
        if (verifyM4) DispatcherQueue.TryEnqueue(() => _ = VerifyM4(root));
        if (m2 is not null) DispatcherQueue.TryEnqueue(() => _ = VerifyM2(root, m2));
#endif
    }

    private void RefreshTick(DispatcherQueueTimer timer, object args) => Run(RefreshNotes);

    internal void CloseSession()
    {
        lifetime?.Cancel();
        if (refreshTimer is not null)
        {
            refreshTimer.Stop();
            refreshTimer.Tick -= RefreshTick;
            refreshTimer = null;
        }
        editor?.Dispose();
        editor = null;
        CloseSync();
        offline?.Dispose();
        offline = null;
    }

    private void RefreshFolders(string id)
    {
        folders = offline!.Call<FolderItem[]>(new { type = "folders" });
        currentFolder = folders.FirstOrDefault(folder => folder.Id == id) ?? folders[0];
        refreshing = true;
        try
        {
            Folders.ItemsSource = folders;
            Folders.SelectedItem = currentFolder;
        }
        finally { refreshing = false; }
        UpdateFolderActions();
    }

    private void UpdateFolderActions()
    {
        RenameFolderButton.IsEnabled = currentFolder?.Kind == "normal";
        NewNoteButton.IsEnabled = currentFolder is not null &&
            (currentFolder.Kind != "trash" || GlobalSearch.Text.Trim().Length > 0);
    }

    private void RefreshNotes()
    {
        if (offline is null || currentFolder is null) return;
        var query = GlobalSearch.Text.Trim();
        var notes = query.Length == 0
            ? offline.Call<NoteItem[]>(new { type = "notes", folder = currentFolder.Id })
            : offline.Call<NoteItem[]>(new { type = "find", query });
        refreshing = true;
        try
        {
            NoteList.ItemsSource = notes;
            GalleryList.ItemsSource = notes;
            NoteList.SelectedItem = notes.FirstOrDefault(note => note.Id == currentNote?.Id);
            GalleryList.SelectedItem = galleryMode ? NoteList.SelectedItem : null;
            NoteList.Visibility = galleryMode ? Visibility.Collapsed : Visibility.Visible;
            GalleryList.Visibility = galleryMode ? Visibility.Visible : Visibility.Collapsed;
        }
        finally { refreshing = false; }
        UpdateFolderActions();
    }

    private void SetCurrent(NoteItem? note)
    {
        if (note is not null && note.Id == currentNote?.Id && note.Deleted == currentNote.Deleted &&
            note.Locked == currentNote.Locked) return;
        if (currentNote is not null && active is not null)
        {
            var (start, end) = Selection(active);
            savedSelections[currentNote.Id] = (active.Id, active.Cell, start, end);
        }
        currentNote = note;
        lastMatch = null;
        lockedSessionNoteId = null;
        if (note?.Locked == true)
        {
            ShowLockScreen(note);
            SetEditLock();
            MoveButton.IsEnabled = note.Deleted == false;
            DeleteButton.IsEnabled = true;
            DeleteButton.Content = note.Deleted ? "Khôi phục" : "Xóa";
            Report();
            return;
        }
        HideLockScreen();
        var snapshot = note is null
            ? new EditorSnapshot(0, [])
            : offline!.Call<EditorSnapshot>(new { type = "open_note", id = note.Id });
        ShowSnapshot(snapshot);
        if (note is not null && savedSelections.TryGetValue(note.Id, out var selection) &&
            views.TryGetValue(selection.Block, out var selectedView))
        {
            active = selectedView.Fields.FirstOrDefault(field => field.Cell == selection.Cell);
            if (active is not null)
            {
                var text = active.Content.Text;
                active.Control.Document.Selection.SetRange(ClampCaret(text, selection.Start), ClampCaret(text, selection.End));
            }
        }
        SetEditLock();
        MoveButton.IsEnabled = note?.Deleted == false;
        DeleteButton.IsEnabled = note is not null;
        DeleteButton.Content = note?.Deleted == true ? "Khôi phục" : "Xóa";
        Report();
    }

    private static int ClampCaret(string text, int offset)
    {
        offset = Math.Min(offset, text.Length);
        return offset < text.Length && char.IsLowSurrogate(text[offset]) ? offset - 1 : offset;
    }

    private void FolderChanged(object sender, SelectionChangedEventArgs args)
    {
        if (refreshing || offline is null || Folders.SelectedItem is not FolderItem selected) return;
        Run(() =>
        {
            if (selected.Id == currentFolder?.Id) return;
            if (!TryCloseEditor())
            {
                refreshing = true;
                try { Folders.SelectedItem = currentFolder; }
                finally { refreshing = false; }
                return;
            }
            currentFolder = selected;
            GlobalSearch.Text = "";
            SetCurrent(null);
            RefreshNotes();
        });
    }

    private void SearchChanged(object sender, TextChangedEventArgs args)
    {
        if (offline is null) return;
        refreshTimer?.Stop();
        refreshTimer?.Start();
    }

    private void NoteChanged(object sender, SelectionChangedEventArgs args)
    {
        if (refreshing || offline is null || NoteList.SelectedItem is not NoteItem selected) return;
        Run(() =>
        {
            if (TryCloseEditor()) SetCurrent(selected);
            else RefreshNotes();
        });
    }

    private async void CreateFolderClick(object sender, RoutedEventArgs args) =>
        await EditFolderName(null);

    private async void RenameFolderClick(object sender, RoutedEventArgs args) =>
        await EditFolderName(currentFolder);

    private async Task EditFolderName(FolderItem? folder)
    {
        if (offline is null || !TryCloseEditor()) return;
        await RunDialog(async () =>
        {
            var input = new TextBox { Header = "Tên thư mục", Text = folder?.Name ?? "", MaxLength = 160 };
            var dialog = Dialog(folder is null ? "Thư mục mới" : "Đổi tên thư mục", input, "Lưu");
            input.TextChanged += (_, _) => dialog.IsPrimaryButtonEnabled = input.Text.Trim().Length > 0;
            dialog.IsPrimaryButtonEnabled = input.Text.Trim().Length > 0;
            if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
            if (folder is null)
            {
                var id = offline.Call<string>(new { type = "create_folder", name = input.Text });
                GlobalSearch.Text = "";
                SetCurrent(null);
                RefreshFolders(id);
            }
            else
            {
                offline.Call(new { type = "rename_folder", id = folder.Id, name = input.Text });
                RefreshFolders(folder.Id);
            }
            RefreshNotes();
        });
    }

    private void CreateNoteClick(object sender, RoutedEventArgs args) => Run(CreateNote);

    private void CreateNote()
    {
        if (offline is null || currentFolder is null || !TryCloseEditor()) return;
        var folder = currentFolder.Kind is "normal" or "inbox" ? currentFolder.Id : "inbox";
        var id = offline.Call<string>(new { type = "create_note", folder });
        if (currentFolder.Kind == "trash") RefreshFolders("all");
        GlobalSearch.Text = "";
        SetCurrent(GetNote(id));
        RefreshNotes();
        views.Values.First().Fields[0].Control.Focus(FocusState.Programmatic);
    }

    private NoteItem GetNote(string id) => offline!.Call<NoteItem>(new { type = "note", id });

    private async void MoveClick(object sender, RoutedEventArgs args)
    {
        if (currentNote is not { Deleted: false } note || !TryCloseEditor()) return;
        await RunDialog(async () =>
        {
            var list = new ComboBox
            {
                Header = "Thư mục đích",
                ItemsSource = folders.Where(folder => folder.Kind is "normal" or "inbox").ToArray(),
                SelectedIndex = 0,
            };
            if (await Dialog("Di chuyển ghi chú", list, "Di chuyển").ShowAsync() != ContentDialogResult.Primary ||
                list.SelectedItem is not FolderItem target) return;
            offline!.Call(new { type = "move_note", id = note.Id, folder = target.Id });
            GlobalSearch.Text = "";
            RefreshFolders(target.Id);
            currentNote = GetNote(note.Id);
            RefreshNotes();
            Report();
        });
    }

    private void DeleteClick(object sender, RoutedEventArgs args) => Run(() =>
    {
        if (currentNote is not { } note || !TryCloseEditor()) return;
        offline!.Call(new { type = "set_deleted", id = note.Id, deleted = !note.Deleted });
        SetCurrent(null);
        RefreshNotes();
    });

    private void ChecklistSortClick(object sender, RoutedEventArgs args) => Run(() =>
    {
        var enabled = ChecklistSort.IsChecked == true;
        if (offline is null || !TryCloseEditor())
        {
            ChecklistSort.IsChecked = !enabled;
            return;
        }
        try { offline.Call(new { type = "set_checklist_sort", enabled }); }
        catch
        {
            ChecklistSort.IsChecked = !enabled;
            throw;
        }
        ArrangeBlocks();
    });

    private void ArrangeBlocks()
    {
        if (prototypeMode) return;
        var ordered = blockOrder.Select(id => views[id]).ToArray();
        if (ChecklistSort.IsChecked == true)
        {
            for (var start = 0; start < ordered.Length;)
            {
                if (ordered[start].Toggle is null)
                {
                    start++;
                    continue;
                }
                var end = start + 1;
                while (end < ordered.Length && ordered[end].Toggle is not null) end++;
                var group = ordered[start..end].OrderBy(view => view.Toggle!.IsChecked == true);
                group.ToArray().CopyTo(ordered, start);
                start = end;
            }
        }
        var focused = active?.Control.FocusState is null or FocusState.Unfocused ? null : active;
        var selection = focused is null ? (0, 0) : Selection(focused);
        var number = 0;
        for (var index = 0; index < ordered.Length; index++)
        {
            var view = ordered[index];
            if (view.Fields.FirstOrDefault() is { } field && field.ParagraphStyle == "numbered")
                field.Control.Header = $"{++number}.";
            else number = 0;
            if (Blocks.Children[index] == view.Root) continue;
            Blocks.Children.Remove(view.Root);
            Blocks.Children.Insert(index, view.Root);
        }
        if (focused is not null && focused.Control.FocusState == FocusState.Unfocused)
        {
            focused.Control.Focus(FocusState.Programmatic);
            focused.Control.Document.Selection.SetRange(selection.Item1, selection.Item2);
        }
    }

    private void SetEditLock()
    {
        foreach (var control in EditActions.Children.OfType<Control>())
            control.IsEnabled = !ReadOnlyNote;
        foreach (var view in views.Values)
        {
            if (view.Toggle is { } toggle) toggle.IsEnabled = !ReadOnlyNote;
            foreach (var field in view.Fields) field.Control.IsReadOnly = ReadOnlyNote;
        }
        RetryButton.Visibility = pending is null ? Visibility.Collapsed : Visibility.Visible;
    }

    private void Commit(Func<EditorUpdate> call, bool redo = false)
    {
        if (pending is not null) throw new InvalidOperationException("Hãy thử lưu lại thay đổi đang chờ.");
        Status.Text = "Đang lưu…";
        EditorUpdate update;
        try
        {
#if DEBUG
            var started = System.Diagnostics.Stopwatch.GetTimestamp();
#endif
            update = call();
#if DEBUG
            acceptanceAcks?.Add(System.Diagnostics.Stopwatch.GetElapsedTime(started).TotalMilliseconds);
#endif
        }
        catch (HistoryConflictException) { throw; }
        catch (Exception error)
        {
            pending = (call, redo);
            SetEditLock();
            throw new InvalidOperationException($"Chưa lưu. {error.Message}", error);
        }
        Project(update, redo);
        ScheduleSyncAfterCommit();
    }

    private void RetryClick(object sender, RoutedEventArgs args) => Run(() =>
    {
        if (pending is not { } retry) return;
        pending = null;
        Commit(retry.Call, retry.Redo);
        SetEditLock();
    });

    internal bool TryCloseEditor()
    {
        if (prototypeMode || offline is null) return true;
        if (pending is not null || loadingImage || IsComposing)
        {
            Report(pending is not null ? "Hãy thử lưu lại trước khi rời ghi chú."
                : loadingImage ? "Đang đọc ảnh từ clipboard…" : "Hãy kết thúc nhập liệu trước khi rời ghi chú.");
            return false;
        }
        try
        {
            foreach (var field in views.Values.SelectMany(view => view.Fields).ToArray())
                SubmitText(field);
            return pending is null;
        }
        catch (Exception error)
        {
            Report(error.Message);
            return false;
        }
    }

    private void FormatClick(object sender, RoutedEventArgs args) =>
        Run(() => Format((string)((Button)sender).Tag));

    private void Format(string style)
    {
        if (active is not { } field || field.Composing || ReadOnlyNote) return;
        var (start, end) = Selection(field);
        if (start == end)
        {
            Report("Bôi đen văn bản để định dạng.");
            return;
        }
        Apply(new { type = "format", id = field.Id, cell = field.Cell, start, end, style });
        field.Control.Focus(FocusState.Programmatic);
    }

    private async void StyleClick(object sender, RoutedEventArgs args)
    {
        if (active is not { Cell: null } field || field.Composing || ReadOnlyNote) return;
        await RunDialog(async () =>
        {
            var names = new[] { "Văn bản", "Tiêu đề", "Đầu đề", "Đầu đề phụ", "Trích dẫn", "Đơn cách",
                "Danh sách dấu chấm", "Danh sách gạch ngang", "Danh sách số", "Checklist" };
            string?[] styles = [null, "title", "heading", "subheading", "quote", "monospaced",
                "bullet", "dashed", "numbered", null];
            var list = new ComboBox { Header = "Kiểu đoạn", ItemsSource = names, SelectedIndex = 0 };
            if (await Dialog("Định dạng đoạn", list, "Áp dụng").ShowAsync() != ContentDialogResult.Primary) return;
            Apply(new { type = "paragraph_style", id = field.Id, style = styles[list.SelectedIndex],
                checklist = list.SelectedIndex == styles.Length - 1 });
            views[field.Id].Fields[0].Control.Focus(FocusState.Programmatic);
        });
    }

    private static void SetParagraphAppearance(Field field, string? style)
    {
        if (field.Cell is not null) return;
        if (field.ParagraphStyle == style) return;
        field.ParagraphStyle = style;
        var box = field.Control;
        box.FontSize = style switch { "title" => 28, "heading" => 22, "subheading" => 18, _ => 15 };
        box.FontWeight = style is "title" or "heading" or "subheading"
            ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal;
        box.FontFamily = new FontFamily(style == "monospaced" ? "Consolas" : "Segoe UI");
        box.Header = style switch
        {
            "quote" => "❝",
            "bullet" => "•",
            "dashed" => "–",
            "numbered" => "1.",
            _ => null,
        };
    }

    private bool HandleOfflineKeys(KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Escape && FindBar.Visibility == Visibility.Visible)
        {
            CloseFind();
            args.Handled = true;
            return true;
        }
        if (!IsDown(VirtualKey.Control) || IsComposing) return false;
        var shift = IsDown(VirtualKey.Shift);
        if (args.Key == VirtualKey.F)
        {
            if (shift || currentNote is null) GlobalSearch.Focus(FocusState.Programmatic);
            else
            {
                FindBar.Visibility = Visibility.Visible;
                FindQuery.Focus(FocusState.Programmatic);
                FindQuery.SelectAll();
            }
        }
        else if (args.Key == VirtualKey.N) Run(CreateNote);
        else if (active?.Control.FocusState is null or FocusState.Unfocused) return false;
        else if (args.Key == VirtualKey.K) _ = ShowLinkPicker(active);
        else if (shift && args.Key == VirtualKey.L && active is { Cell: null } field && !ReadOnlyNote)
            Run(() => ToggleChecklist(field));
        else if (args.Key is VirtualKey.B or VirtualKey.I or VirtualKey.U)
            Run(() => Format(args.Key switch { VirtualKey.B => "bold", VirtualKey.I => "italic", _ => "underline" }));
        else return false;
        args.Handled = true;
        return true;
    }

    private void ToggleChecklist(Field field)
    {
        if (field.Checklist) Apply(new { type = "toggle", id = field.Id });
        else Apply(new { type = "paragraph_style", id = field.Id, style = (string?)null, checklist = true });
    }

    private void FindKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key != VirtualKey.Enter) return;
        args.Handled = true;
        Run(FindNext);
    }

    private void FindClick(object sender, RoutedEventArgs args) => Run(FindNext);

    private void CloseFindClick(object sender, RoutedEventArgs args) => CloseFind();

    private void CloseFind()
    {
        FindBar.Visibility = Visibility.Collapsed;
        lastMatch = null;
        active?.Control.Focus(FocusState.Programmatic);
    }

    private void FindNext()
    {
        var query = FindQuery.Text.Trim();
        if (offline is null || query.Length == 0) return;
        var fields = Blocks.Children.OfType<FrameworkElement>()
            .SelectMany(root => views.Values.First(view => view.Root == root).Fields);
        var matches = fields.SelectMany(field => offline.Call<TextMatch[]>(new
            { type = "find_in_text", text = ReadText(field), query })
            .Select(match => (Field: field, Match: match))).ToArray();
        if (matches.Length == 0)
        {
            Report("Không tìm thấy trong ghi chú.");
            return;
        }
        var index = Array.FindIndex(matches, item =>
            lastMatch == (item.Field.Id, item.Field.Cell, item.Match.Start, item.Match.End, query));
        var (field, match) = matches[(index + 1) % matches.Length];
        lastMatch = (field.Id, field.Cell, match.Start, match.End, query);
        field.Control.Focus(FocusState.Programmatic);
        field.Control.Document.Selection.SetRange(match.Start, match.End);
    }

    private async void LinkClick(object sender, RoutedEventArgs args)
    {
        if (active is { } field) await ShowLinkPicker(field);
    }

    private void SuggestAfterTrigger(Field field, int end)
    {
        if (field.Cell is not null || dialogOpen || ReadOnlyNote) return;
        if (end < 2 || end > field.Content.Text.Length ||
            field.Content.Text.AsSpan(end - 2, 2) is not ['>', '>']) return;
        DispatcherQueue.TryEnqueue(() =>
        {
            if (active == field && Selection(field) == (end, end))
                _ = ShowLinkPicker(field, end - 2);
        });
    }

    private async Task ShowLinkPicker(Field field, int? trigger = null)
    {
        if (offline is null || currentNote is null || field.Cell is not null || ReadOnlyNote || field.Composing) return;
        var (start, end) = Selection(field);
        await RunDialog(async () =>
        {
            var text = new TextBox { PlaceholderText = "Tìm ghi chú đích" };
            var targets = new ListView { MaxHeight = 230 };
            var label = new TextBox { PlaceholderText = "Nhãn tùy chỉnh (không bắt buộc)" };
            var content = new StackPanel { Spacing = 8, Children = { text, targets, label } };
            var dialog = Dialog("Chèn liên kết ghi chú", content, "Chèn");
            void Filter()
            {
                var query = text.Text.Trim();
                var notes = query.Length == 0
                    ? offline.Call<NoteItem[]>(new { type = "notes", folder = "all" })
                    : offline.Call<NoteItem[]>(new { type = "find", query });
                targets.ItemsSource = notes.Where(note => note.Id != currentNote.Id).ToArray();
                targets.SelectedIndex = targets.Items.Count > 0 ? 0 : -1;
                dialog.IsPrimaryButtonEnabled = targets.SelectedIndex >= 0;
            }
            Filter();
            text.TextChanged += (_, _) => Run(Filter);
            targets.SelectionChanged += (_, _) => dialog.IsPrimaryButtonEnabled = targets.SelectedIndex >= 0;
            text.KeyDown += (_, args) =>
            {
                if (args.Key is not (VirtualKey.Down or VirtualKey.Up) || targets.Items.Count == 0) return;
                var step = args.Key == VirtualKey.Down ? 1 : -1;
                targets.SelectedIndex = Math.Clamp(targets.SelectedIndex + step, 0, targets.Items.Count - 1);
                targets.ScrollIntoView(targets.SelectedItem);
                args.Handled = true;
            };
            if (await dialog.ShowAsync() != ContentDialogResult.Primary || targets.SelectedItem is not NoteItem target) return;
            InsertNoteLink(field, trigger ?? start, end, target, label.Text);
        });
    }

    private void InsertNoteLink(Field field, int start, int end, NoteItem target, string? label)
    {
        var id = Guid.NewGuid().ToString("N");
        var tail = Guid.NewGuid().ToString("N");
        Apply(new { type = "insert_at", id = field.Id, start, end,
            tail_id = tail, block = new { id, kind = "note_link", note_id = target.Id,
                label = string.IsNullOrWhiteSpace(label) ? null : label.Trim() } });
        views[tail].Fields[0].Control.Focus(FocusState.Programmatic);
    }

    private void OpenLinkedNote(string id)
    {
        if (!TryCloseEditor()) return;
        var next = GetNote(id);
        if (next.Deleted)
        {
            Report("Ghi chú đích đã bị xóa.");
            return;
        }
        GlobalSearch.Text = "";
        RefreshFolders(next.FolderId);
        SetCurrent(next);
        RefreshNotes();
    }

    // ---- Tổ chức nâng cao: bảng, tag, Smart Folders, Gallery (M4-W.4) ----

    private bool galleryMode;

    private void InsertTableClick(object sender, RoutedEventArgs args)
    {
        if (offline is null || currentNote is null || active is null || ReadOnlyNote || IsComposing) return;
        if (active.Cell is not null)
        {
            Report("Đang ở trong ô bảng rồi.");
            return;
        }
        var (start, end) = Selection(active);
        var table = new
        {
            id = Guid.NewGuid().ToString("N"),
            kind = "table",
            rows = new[]
            {
                new[] { new { text = "", marks = Array.Empty<object>() }, new { text = "", marks = Array.Empty<object>() } },
                new[] { new { text = "", marks = Array.Empty<object>() }, new { text = "", marks = Array.Empty<object>() } },
            },
        };
        Run(() => Apply(new
        {
            type = "insert_at",
            id = active.Id,
            start,
            end,
            block = table,
            tail_id = Guid.NewGuid().ToString("N"),
        }));
        Report("Đã chèn bảng 2×2.");
    }

    private void TableOpClick(object sender, RoutedEventArgs args)
    {
        var field = active;
        if (offline is null || field?.Cell is not { } cell || ReadOnlyNote || IsComposing) return;
        var code = (string)((Button)sender).Tag;
        object? operation = code switch
        {
            "table_insert_row" => new { type = code, id = field.Id, after_row = (int?)cell.Row },
            "table_remove_row" => new { type = code, id = field.Id, row = cell.Row },
            "table_insert_column" => new { type = code, id = field.Id, after_column = (int?)cell.Column },
            "table_remove_column" => new { type = code, id = field.Id, column = cell.Column },
            "table_move_row_up" when cell.Row > 0 =>
                new { type = "table_move_row", id = field.Id, from = cell.Row, to = cell.Row - 1 },
            "table_move_row_down" =>
                new { type = "table_move_row", id = field.Id, from = cell.Row, to = cell.Row + 1 },
            "table_move_column_left" when cell.Column > 0 =>
                new { type = "table_move_column", id = field.Id, from = cell.Column, to = cell.Column - 1 },
            "table_move_column_right" =>
                new { type = "table_move_column", id = field.Id, from = cell.Column, to = cell.Column + 1 },
            _ => null,
        };
        if (operation is null)
        {
            Report("Không di chuyển thêm được nữa.");
            return;
        }
        Run(() => Apply(operation));
    }

    private async void CreateSmartFolderClick(object sender, RoutedEventArgs args)
    {
        if (offline is null) return;
        await RunDialog(async () =>
        {
            var name = new TextBox { Header = "Tên Smart Folder", MaxLength = 160 };
            var tag = new TextBox { Header = "Tag (không cần #)", PlaceholderText = "cong-viec" };
            var checklist = new CheckBox { Content = "Có checklist" };
            var attachments = new CheckBox { Content = "Có đính kèm" };
            var panel = new StackPanel { Spacing = 12 };
            panel.Children.Add(name);
            panel.Children.Add(tag);
            panel.Children.Add(checklist);
            panel.Children.Add(attachments);
            var dialog = Dialog("Smart Folder mới", panel, "Tạo");
            var enabled = () => dialog.IsPrimaryButtonEnabled =
                name.Text.Trim().Length > 0 &&
                (tag.Text.Trim().Length > 0 || checklist.IsChecked == true || attachments.IsChecked == true);
            name.TextChanged += (_, _) => enabled();
            tag.TextChanged += (_, _) => enabled();
            checklist.Click += (_, _) => enabled();
            attachments.Click += (_, _) => enabled();
            dialog.IsPrimaryButtonEnabled = false;
            if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
            var query = System.Text.Json.JsonSerializer.Serialize(new
            {
                tag = tag.Text.Trim() is { Length: > 0 } value ? value : null,
                has_checklist = checklist.IsChecked == true ? (bool?)true : null,
                has_attachments = attachments.IsChecked == true ? (bool?)true : null,
                folder = (string?)null,
            });
            var id = offline.Call<string>(new { type = "create_smart_folder", name = name.Text, query });
            GlobalSearch.Text = "";
            RefreshFolders(id);
            SetCurrent(null);
            RefreshTags();
            RefreshNotes();
            Report("Đã tạo Smart Folder.");
        });
    }

    private sealed record TagCountDto(string Tag, long Notes)
    {
        public override string ToString() => $"#{Tag} ({Notes})";
    }

    private void RefreshTags()
    {
        if (offline is null) return;
        refreshing = true;
        try
        {
            TagList.ItemsSource = offline.Call<TagCountDto[]>(new { type = "tags_list" });
        }
        finally { refreshing = false; }
    }

    private void TagChanged(object sender, SelectionChangedEventArgs args)
    {
        if (refreshing || offline is null || TagList.SelectedItem is not TagCountDto tag) return;
        GlobalSearch.Text = $"#{tag.Tag}";
        GlobalSearch.Focus(FocusState.Programmatic);
    }

    private void ViewToggleClick(object sender, RoutedEventArgs args)
    {
        galleryMode = !galleryMode;
        ViewToggle.Content = galleryMode ? "Danh sách" : "Gallery";
        RefreshNotes();
    }

    // ---- Import/Export (M4-W.5) ----

    private async Task<string?> PickFolderAsync()
    {
        var picker = new Windows.Storage.Pickers.FolderPicker
        {
            SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary,
        };
        picker.FileTypeFilter.Add("*");
        var window = App.MainWindow ?? throw new InvalidOperationException("Cửa sổ chưa sẵn sàng.");
        WinRT.Interop.InitializeWithWindow.Initialize(picker,
            WinRT.Interop.WindowNative.GetWindowHandle(window));
        var folder = await picker.PickSingleFolderAsync();
        return folder?.Path;
    }

    private async void ExportClick(object sender, RoutedEventArgs args)
    {
        if (offline is null || currentNote is not { } note || note.Locked) return;
        await RunDialog(async () =>
        {
            var format = new ComboBox
            {
                Header = "Định dạng",
                ItemsSource = new[] { "Markdown", "HTML", "PDF" },
                SelectedIndex = 0,
            };
            if (await Dialog("Xuất ghi chú", format, "Xuất").ShowAsync() != ContentDialogResult.Primary)
                return;
            var dir = await PickFolderAsync();
            if (dir is null) return;
            var fmt = ((string)format.SelectedItem).ToLowerInvariant();
            var path = offline.Call<string>(new { type = "export_note", id = note.Id, format = fmt, dir });
            Report($"Đã xuất: {path}");
        });
    }

    private async void ImportClick(object sender, RoutedEventArgs args)
    {
        if (offline is null) return;
        await RunDialog(async () =>
        {
            var format = new ComboBox
            {
                Header = "Nguồn",
                ItemsSource = new[] { "Markdown folder", "Apple Notes (HTML)" },
                SelectedIndex = 0,
            };
            if (await Dialog("Nhập ghi chú", format, "Nhập").ShowAsync() != ContentDialogResult.Primary)
                return;
            var dir = await PickFolderAsync();
            if (dir is null) return;
            var fmt = ((string)format.SelectedItem).StartsWith("Markdown") ? "markdown" : "apple_html";
            var count = offline.Call<string[]>(new { type = "import_folder", path = dir, parent = (string?)null, format = fmt });
            GlobalSearch.Text = "";
            RefreshFolders("all");
            RefreshNotes();
            RefreshTags();
            Report($"Đã nhập {count.Length} ghi chú.");
        });
    }

    // ---- Locked Notes (M4-W.2) ----

    internal bool IsLockedSession => currentNote is not null && lockedSessionNoteId == currentNote.Id;

    private void ShowLockScreen(NoteItem note)
    {
        ShowSnapshot(new EditorSnapshot(0, []));
        EditorScroll.Visibility = Visibility.Collapsed;
        LockScreen.Visibility = Visibility.Visible;
        LockTitle.Text = note.DisplayTitle;
        LockPassphrase.Password = "";
        LockPassphrase.IsEnabled = true;
        UnlockButton.IsEnabled = true;
        RelockButton.Visibility = Visibility.Collapsed;
        LockHint.Visibility = Visibility.Collapsed;
        LockHint.Text = "";
        try
        {
            if (offline!.Call<string?>(new { type = "lock_hint", id = note.Id }) is { Length: > 0 } hint)
            {
                LockHint.Text = $"Gợi ý: {hint}";
                LockHint.Visibility = Visibility.Visible;
            }
        }
        catch (Exception) { /* gợi ý chỉ mang tính phụ trợ */ }
        _ = ShowHelloUnlockAsync();
        Report("Ghi chú đang khóa.");
    }

    private void HideLockScreen()
    {
        LockScreen.Visibility = Visibility.Collapsed;
        EditorScroll.Visibility = Visibility.Visible;
        LockPassphrase.Password = "";
        RelockButton.Visibility = Visibility.Collapsed;
    }

    private async Task ShowHelloUnlockAsync()
    {
        HelloUnlockButton.Visibility = Visibility.Collapsed;
        if (dataRoot is null) return;
        var remembered = await HelloUnlockStore.LoadAsync(dataRoot);
        if (remembered is not null && await HelloUnlock.IsAvailableAsync())
            HelloUnlockButton.Visibility = Visibility.Visible;
    }

    private async void LockClick(object sender, RoutedEventArgs args)
    {
        if (offline is null || currentNote is not { Deleted: false, Locked: false } note || !TryCloseEditor()) return;
        if (!lockSupported)
        {
            Report("Kho dữ liệu chưa hỗ trợ khóa ghi chú (cần SQLCipher).");
            return;
        }
        await RunDialog(async () =>
        {
            var passphrase = new PasswordBox { Header = "Mật khẩu khóa" };
            var hint = new TextBox { Header = "Gợi ý (không bắt buộc)", MaxLength = 160 };
            var hello = new CheckBox { Content = "Cho phép mở nhanh bằng Windows Hello",
                IsEnabled = await HelloUnlock.IsAvailableAsync() };
            var panel = new StackPanel { Spacing = 12 };
            panel.Children.Add(passphrase);
            panel.Children.Add(hint);
            panel.Children.Add(hello);
            var dialog = Dialog("Khóa ghi chú", panel, "Khóa");
            passphrase.PasswordChanged += (_, _) =>
                dialog.IsPrimaryButtonEnabled = passphrase.Password.Length > 0;
            dialog.IsPrimaryButtonEnabled = false;
            if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
            offline.Call(new { type = "lock_note", id = note.Id, passphrase = passphrase.Password,
                hint = string.IsNullOrWhiteSpace(hint.Text) ? null : hint.Text.Trim() });
            if (hello.IsChecked == true && await HelloUnlock.IsAvailableAsync())
                await HelloUnlockStore.SaveAsync(dataRoot!, passphrase.Password);
            else
                await HelloUnlockStore.DeleteAsync(dataRoot!);
            currentNote = GetNote(note.Id);
            RefreshNotes();
            SetCurrent(currentNote);
            Report("Ghi chú đã được khóa.");
        });
    }

    private async void UnlockClick(object sender, RoutedEventArgs args) =>
        await UnlockWith(LockPassphrase.Password);

    private async void HelloUnlockClick(object sender, RoutedEventArgs args)
    {
        if (dataRoot is null) return;
        if (!await HelloUnlock.ConsentAsync("Mở khóa ghi chú trong Notes"))
        {
            Report("Windows Hello không xác nhận được. Hãy dùng mật khẩu khóa.");
            return;
        }
        var passphrase = await HelloUnlockStore.LoadAsync(dataRoot);
        if (passphrase is null)
        {
            Report("Không đọc được mật khẩu đã nhớ cho Windows Hello.");
            return;
        }
        await UnlockWith(passphrase);
    }

    private async Task UnlockWith(string passphrase)
    {
        if (offline is null || currentNote is not { } note) return;
        if (passphrase.Length == 0)
        {
            Report("Nhập mật khẩu khóa.");
            return;
        }
        try
        {
            var snapshot = offline.Call<EditorSnapshot>(new { type = "unlock_note", id = note.Id, passphrase });
            HideLockScreen();
            ShowSnapshot(snapshot);
            lockedSessionNoteId = note.Id;
            RelockButton.Visibility = Visibility.Visible;
            Report("Ghi chú đã mở khóa cho phiên này.");
            ResetAutoLock();
        }
        catch (WrongPassphraseException error) { Report(error.Message); }
        catch (InvalidOperationException error) { Report(error.Message); }
        await Task.CompletedTask;
    }

    private void LockPassphraseKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Enter)
        {
            args.Handled = true;
            Run(() => _ = UnlockWith(LockPassphrase.Password));
        }
    }

    private void RelockClick(object sender, RoutedEventArgs args) => Run(LockAllLocked);

    /// Khóa lại mọi phiên mở: bỏ NLK và plaintext khỏi bộ nhớ của core.
    internal void LockAllLocked()
    {
        if (offline is null || lockedSessionNoteId is null) return;
        var id = lockedSessionNoteId;
        offline.Call(new { type = "relock_note", id });
        lockedSessionNoteId = null;
        RefreshNotes();
        currentNote = GetNote(id);
        SetCurrent(currentNote);
        Report("Đã khóa lại ghi chú.");
    }

    private void SetupAutoLock()
    {
        autoLockTimer ??= DispatcherQueue.CreateTimer();
        autoLockTimer.Interval = TimeSpan.FromMinutes(AutoLockMinutes);
        autoLockTimer.IsRepeating = false;
        autoLockTimer.Tick += (_, _) => DispatcherQueue.TryEnqueue(LockAllLocked);
        ResetAutoLock();
        // Ngủ máy (WM_POWERBROADCAST PBT_APMSUSPEND) và khóa phiên (WTS_SESSION_LOCK)
        // đều đến từ SessionLockMonitor để tránh phụ thuộc PowerManager của WinAppSDK.
        SessionLockMonitor.Start(() => DispatcherQueue.TryEnqueue(LockAllLocked));
    }

    internal void ResetAutoLock()
    {
        if (!IsLockedSession) return;
        autoLockTimer?.Stop();
        autoLockTimer?.Start();
    }

    private FrameworkElement CreateEmbed(Block block)
    {
        var row = new StackPanel { Spacing = 6, Orientation = Orientation.Horizontal };
        if (block.Kind == "note_link")
        {
            NoteItem? target = null;
            try { target = GetNote(block.NoteId!); }
            catch (InvalidOperationException) { }
            var link = new Button
            {
                Content = block.Label ?? target?.DisplayTitle ?? "Ghi chú không còn tồn tại",
                IsEnabled = target?.Deleted == false,
            };
            link.Click += (_, _) => Run(() => OpenLinkedNote(block.NoteId!));
            row.Children.Add(link);
        }
        else if (IsLockedSession)
        {
            var image = new Image { MaxHeight = 260, MaxWidth = 480, Stretch = Stretch.Uniform };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(image, "Ảnh đính kèm");
            var message = new TextBlock { Text = "Đang tải ảnh…" };
            row.Children.Add(image);
            row.Children.Add(message);
            try
            {
                var bytes = offline!.Call<ImageDataDto>(new { type = "image_data", id = block.AttachmentId });
                _ = LoadImageBytes(image, message, bytes.Data);
            }
            catch (Exception error) { message.Text = error.Message; }
        }
        else
        {
            var image = new Image { MaxHeight = 260, MaxWidth = 480, Stretch = Stretch.Uniform };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(image, "Ảnh đính kèm");
            var message = new TextBlock { Text = "Đang tải ảnh…" };
            row.Children.Add(image);
            row.Children.Add(message);
            try
            {
                var path = offline!.Call<string>(new { type = "image_path", id = block.AttachmentId });
                _ = LoadImage(image, message, path);
            }
            catch (Exception error) { message.Text = error.Message; }
        }
        if (currentNote?.Deleted == false)
        {
            var remove = new Button { Content = "Gỡ", VerticalAlignment = VerticalAlignment.Top };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(remove, block.Kind == "image" ? "Gỡ ảnh" : "Gỡ liên kết");
            remove.Click += (_, _) => Run(() =>
            {
                if (TryCloseEditor()) Apply(new { type = "remove_embed", id = block.Id });
            });
            row.Children.Add(remove);
        }
        return row;
    }

    private static async Task LoadImageBytes(Image image, TextBlock message, string base64)
    {
        try
        {
            var bytes = Convert.FromBase64String(base64);
            using var stream = new InMemoryRandomAccessStream();
            using (var writer = new DataWriter(stream))
            {
                writer.WriteBytes(bytes);
                await writer.StoreAsync();
                writer.DetachStream();
            }
            stream.Seek(0);
            var bitmap = new BitmapImage { DecodePixelWidth = 960 };
            await bitmap.SetSourceAsync(stream);
            image.Source = bitmap;
            message.Visibility = Visibility.Collapsed;
        }
        catch (Exception) { message.Text = "Không đọc được ảnh đính kèm."; }
    }

    private static async Task LoadImage(Image image, TextBlock message, string path)
    {
        try
        {
            var file = await Windows.Storage.StorageFile.GetFileFromPathAsync(path);
            using var stream = await file.OpenAsync(Windows.Storage.FileAccessMode.Read);
            var bitmap = new BitmapImage { DecodePixelWidth = 960 };
            await bitmap.SetSourceAsync(stream);
            image.Source = bitmap;
            message.Visibility = Visibility.Collapsed;
        }
        catch (Exception) { message.Text = "Không đọc được ảnh đính kèm."; }
    }

    private async void Paste(Field field, TextControlPasteEventArgs args)
    {
        if (ReadOnlyNote || loadingImage || field.Composing || field.Cell is not null) return;
        try
        {
            var clipboard = Clipboard.GetContent();
            if (!clipboard.Contains(StandardDataFormats.Bitmap)) return;
            args.Handled = true;
            await InsertClipboardImage(field, clipboard);
        }
        catch (Exception error) { Report(error.Message); }
    }

    private async Task InsertClipboardImage(Field field, DataPackageView clipboard)
    {
        var (start, end) = Selection(field);
        var expected = revision;
        var note = currentNote?.Id;
        loadingImage = true;
        try
        {
            var reference = await clipboard.GetBitmapAsync();
            using var source = await reference.OpenReadAsync();
            var decoder = await BitmapDecoder.CreateAsync(source);
            if ((ulong)decoder.PixelWidth * decoder.PixelHeight > 40_000_000)
                throw new InvalidOperationException("Ảnh quá lớn.");
            using var bitmap = await decoder.GetSoftwareBitmapAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied);
            using var output = new InMemoryRandomAccessStream();
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, output);
            encoder.SetSoftwareBitmap(bitmap);
            await encoder.FlushAsync();
            if (output.Size > 32 * 1024 * 1024) throw new InvalidOperationException("Ảnh vượt quá 32 MB.");
            using var reader = new DataReader(output.GetInputStreamAt(0));
            var bytes = new byte[(int)output.Size];
            await reader.LoadAsync((uint)bytes.Length);
            reader.ReadBytes(bytes);
            if (expected != revision || note != currentNote?.Id)
                throw new InvalidOperationException("Vị trí đã thay đổi. Hãy dán lại ảnh.");
            var png = Convert.ToBase64String(bytes);
            Commit(() => offline!.Call<EditorUpdate>(new
            {
                type = "paste_image", revision = expected, id = field.Id, start, end, png,
            }));
            var index = blockOrder.IndexOf(field.Id);
            if (index + 2 < blockOrder.Count)
            {
                var tail = views[blockOrder[index + 2]];
                tail.Fields.FirstOrDefault()?.Control.Focus(FocusState.Programmatic);
            }
        }
        finally { loadingImage = false; }
    }

    private static (int Start, int End) Selection(Field field)
    {
        var selection = field.Control.Document.Selection;
        var length = field.Content.Text.Length;
        return (Math.Clamp(selection.StartPosition, 0, length), Math.Clamp(selection.EndPosition, 0, length));
    }

    private ContentDialog Dialog(string title, FrameworkElement content, string primary) => new()
    {
        XamlRoot = XamlRoot, Title = title, Content = content, PrimaryButtonText = primary,
        CloseButtonText = "Hủy", DefaultButton = ContentDialogButton.Primary,
    };

    private async Task RunDialog(Func<Task> action)
    {
        if (dialogOpen) return;
        dialogOpen = true;
        try { await action(); }
        catch (Exception error) { Report(error.Message); }
        finally { dialogOpen = false; }
    }
}

