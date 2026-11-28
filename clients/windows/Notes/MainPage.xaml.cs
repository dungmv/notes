using System.Diagnostics;
using System.Text.Json;
using Microsoft.UI.Input;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;
using Windows.UI.Core;

namespace Notes;

public sealed partial class MainPage : Page
{
    private sealed record View(FrameworkElement Root, CheckBox? Toggle, Field[] Fields, string Kind);

    private sealed class Field(string id, Cell? cell)
    {
        internal string Id { get; } = id;
        internal Cell? Cell { get; } = cell;
        internal RichEditBox Control { get; } = new() { MinHeight = 42, AcceptsReturn = true };
        internal RichText Content { get; set; } = new("", []);
        internal bool Composing { get; set; }
        internal int CompositionStart { get; set; }
        internal bool Checklist { get; init; }
        internal string? ParagraphStyle { get; set; }
        internal long Started { get; set; }
    }

    private readonly Dictionary<string, View> views = [];
    private readonly List<string> blockOrder = [];
    private readonly Queue<(Field Field, int Offset)> deferred = [];
    private readonly List<double> typing = [];
    private readonly List<double> patches = [];
    private PrototypeEditor? editor;
    private Field? active;
    private ulong revision;
    private bool projecting;

    public MainPage()
    {
        InitializeComponent();
        Loaded += (_, _) => InitializeEditor();
        Unloaded += (_, _) => CloseSession();
        PreviewKeyDown += PageKeys;
        PointerMoved += (_, _) => ResetAutoLock();
        PointerPressed += (_, _) => ResetAutoLock();
        Status.Text = "Đang mở ghi chú…";
    }

    private void Reset()
    {
        editor?.Dispose();
        editor = PrototypeEditor.Open();
        ShowSnapshot(editor.Snapshot());
        Report();
    }

    private void ShowSnapshot(EditorSnapshot snapshot)
    {
        revision = snapshot.Revision;
        projecting = true;
        try
        {
            deferred.Clear();
            active = null;
            typing.Clear();
            patches.Clear();
            views.Clear();
            blockOrder.Clear();
            Blocks.Children.Clear();
            foreach (var block in snapshot.Blocks)
            {
                blockOrder.Add(block.Id);
                Upsert(Blocks.Children.Count, block);
            }
            ArrangeBlocks();
        }
        finally { projecting = false; }
    }

    private void Upsert(int index, Block block, bool redo = false)
    {
        Field? previous = null;
        // Bảng đổi cấu trúc (thêm/xóa hàng-cột) phải dựng lại lưới ô, không
        // chỉ chiếu nội dung vào các ô cũ.
        var rebuild = views.TryGetValue(block.Id, out var existing) &&
            (existing.Kind != block.Kind ||
             (block.Rows is { } dims && existing.Fields.Length != dims.Sum(row => row.Length)));
        if (rebuild && existing is not null)
        {
            if (existing.Fields.Contains(active)) previous = active;
            Blocks.Children.Remove(existing.Root);
            views.Remove(block.Id);
        }
        if (!views.TryGetValue(block.Id, out var view))
        {
            if (block.Kind is "image" or "note_link")
            {
                var embed = CreateEmbed(block);
                views.Add(block.Id, new(embed, null, [], block.Kind));
                Blocks.Children.Insert(index, embed);
                return;
            }
            var grid = new Grid { ColumnSpacing = 6, RowSpacing = 6 };
            CheckBox? toggle = null;
            var fields = new List<Field>();
            if (block.Rows is { } rows)
            {
                foreach (var _ in rows[0]) grid.ColumnDefinitions.Add(new ColumnDefinition());
                for (var row = 0; row < rows.Length; row++)
                {
                    grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
                    for (var column = 0; column < rows[row].Length; column++)
                    {
                        var field = NewField(block.Id, new(row, column), false);
                        field.Control.Header = $"R{row + 1} C{column + 1}";
                        Grid.SetRow(field.Control, row);
                        Grid.SetColumn(field.Control, column);
                        grid.Children.Add(field.Control);
                        fields.Add(field);
                    }
                }
            }
            else
            {
                var checklist = block.Kind == "checklist";
                var field = NewField(block.Id, null, checklist);
                if (checklist)
                {
                    toggle = new CheckBox { MinWidth = 0, IsEnabled = !ReadOnlyNote };
                    Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(toggle, $"Toggle {block.Id}");
                    toggle.Click += (_, _) => Run(() => Apply(new { type = "toggle", id = block.Id }));
                    grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                    grid.ColumnDefinitions.Add(new ColumnDefinition());
                    grid.Children.Add(toggle);
                    Grid.SetColumn(field.Control, 1);
                }
                grid.Children.Add(field.Control);
                fields.Add(field);
            }
            view = new(grid, toggle, [.. fields], block.Kind);
            views.Add(block.Id, view);
            Blocks.Children.Insert(index, grid);
        }

        view.Root.Margin = new Thickness(block.Depth * 24, 0, 0, 0);
        if (view.Toggle is { } box) box.IsChecked = block.Checked;
        foreach (var field in view.Fields)
        {
            SetParagraphAppearance(field, block.Style);
            var content = field.Cell is { } cell ? block.Rows![cell.Row][cell.Column] : block.Content!;
            if (field.Content.Text != content.Text || !field.Content.Marks.SequenceEqual(content.Marks))
                Project(field, content, redo && field == active);
        }
        if (previous is not null)
        {
            var old = previous.Control.Document.Selection;
            active = view.Fields[0];
            active.Control.Focus(FocusState.Programmatic);
            active.Control.Document.Selection.SetRange(
                Math.Min(old.StartPosition, active.Content.Text.Length),
                Math.Min(old.EndPosition, active.Content.Text.Length));
        }
    }

    private Field NewField(string id, Cell? cell, bool checklist)
    {
        var field = new Field(id, cell) { Checklist = checklist };
        var box = field.Control;
        box.IsReadOnly = ReadOnlyNote;
        if (!prototypeMode)
        {
            box.Paste += (_, args) => Paste(field, args);
            box.MinHeight = 36;
        }
        var automationName = prototypeMode ? (cell is null ? id : $"{id} {cell.Row} {cell.Column}")
            : cell is null ? "Nội dung ghi chú" : $"Ô hàng {cell.Row + 1}, cột {cell.Column + 1}";
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, automationName);
        box.GotFocus += (_, _) => active = field;
        box.TextChanging += (_, _) =>
        {
            if (!projecting && field.Started == 0) field.Started = Stopwatch.GetTimestamp();
        };
        box.TextChanged += (_, _) =>
        {
            if (!projecting && !field.Composing) Run(() => SubmitText(field));
        };
        box.TextCompositionStarted += (_, _) =>
        {
            field.Composing = true;
            field.CompositionStart = box.Document.Selection.StartPosition;
            Report();
        };
        box.TextCompositionChanged += (_, args) => field.CompositionStart = args.StartIndex;
        box.TextCompositionEnded += (_, _) =>
        {
            field.Composing = false;
            field.Started = Stopwatch.GetTimestamp();
            Run(() =>
            {
                SubmitText(field);
                Report();
            });
            // The key that committed the composition can still insert a delimiter.
            // Project queued patches after RichEditBox finishes handling that key.
            if (deferred.Count != 0)
            {
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low,
                    () => Run(FlushDeferred));
            }
            if (pendingSyncReload && !IsComposing)
            {
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low,
                    () => Run(ReloadCurrentNotePreservingSelection));
            }
        };
        return field;
    }

    private void SubmitText(Field field)
    {
        var started = field.Started;
        field.Started = 0;
        var current = ReadText(field);
        var old = field.Content.Text;
        if (current == old || (editor is null && offline is null) || ReadOnlyNote) return;
        var change = Difference(old, current);
        if (!prototypeMode && field.Cell is null && !field.Checklist && current == "[] ")
        {
            ApplyBatch([Replace(field, 0, old.Length, ""), new { type = "paragraph_style",
                id = field.Id, style = (string?)null, checklist = true }]);
            return;
        }
        Apply(Replace(field, change.Start, change.End, change.Text));
#if DEBUG
        if (started != 0) acceptanceProjections?.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
#endif
        if (!prototypeMode && change.Text.EndsWith('>'))
            SuggestAfterTrigger(field, change.Start + change.Text.Length);
        if (prototypeMode && started != 0) typing.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
        Report();
    }

    private static string ReadText(Field field)
    {
        field.Control.Document.GetText(TextGetOptions.None, out var current);
        // TOM includes an implicit end-of-document paragraph.
        return (current.EndsWith('\r') ? current[..^1] : current).Replace('\r', '\n');
    }

    private static (int Start, int End, string Text) Difference(string old, string current)
    {
        var start = 0;
        while (start < old.Length && start < current.Length && old[start] == current[start]) start++;
        if (start > 0 && (start < old.Length && char.IsLowSurrogate(old[start]) ||
                         start < current.Length && char.IsLowSurrogate(current[start]))) start--;
        var end = old.Length;
        var nextEnd = current.Length;
        while (end > start && nextEnd > start && old[end - 1] == current[nextEnd - 1])
        {
            end--;
            nextEnd--;
        }
        if (end < old.Length && char.IsLowSurrogate(old[end]))
        {
            end++;
            nextEnd++;
        }
        return (start, end, current[start..nextEnd]);
    }

    private static void Project(Field field, RichText content, bool advanceCaret)
    {
        // RichEditBox blocks programmatic document writes while it is read-only.
        var readOnly = field.Control.IsReadOnly;
        if (readOnly) field.Control.IsReadOnly = false;
        try
        {
            var doc = field.Control.Document;
            var before = ReadText(field);
            var selection = doc.Selection;
            var anchor = selection.StartPosition;
            var end = selection.EndPosition;
            if (before != content.Text)
            {
                var change = Difference(before, content.Text);
                doc.GetRange(change.Start, change.End).SetText(TextSetOptions.None, change.Text.Replace('\n', '\r'));
                // A collapsed caret follows text restored by redo at its insertion point.
                var followInsertion = advanceCaret && anchor == end && anchor == change.Start && change.Start == change.End;
                int Translate(int offset) => followInsertion ? offset + change.Text.Length :
                    offset <= change.Start ? offset :
                    offset >= change.End ? offset + change.Text.Length - (change.End - change.Start) :
                    change.Start + change.Text.Length;
                anchor = Translate(anchor);
                end = Translate(end);
            }

            var format = doc.GetRange(0, content.Text.Length).CharacterFormat;
            format.Bold = FormatEffect.Off;
            format.Italic = FormatEffect.Off;
            format.Underline = UnderlineType.None;
            format.Strikethrough = FormatEffect.Off;
            foreach (var mark in content.Marks)
            {
                var marked = doc.GetRange(mark.Start, mark.End).CharacterFormat;
                if (mark.Style == "bold") marked.Bold = FormatEffect.On;
                if (mark.Style == "italic") marked.Italic = FormatEffect.On;
                if (mark.Style == "underline") marked.Underline = UnderlineType.Single;
                if (mark.Style == "strikethrough") marked.Strikethrough = FormatEffect.On;
            }
            selection.SetRange(Math.Clamp(anchor, 0, content.Text.Length), Math.Clamp(end, 0, content.Text.Length));
            field.Content = content;
        }
        finally
        {
            if (readOnly) field.Control.IsReadOnly = true;
        }
    }

    private void Project(EditorUpdate update, bool redo = false)
    {
        if (update.BaseRevision != revision) throw new InvalidOperationException("Patch revision mismatch.");
        var started = Stopwatch.GetTimestamp();
        projecting = true;
        try
        {
            foreach (var patch in update.Patches)
            {
                if (patch.Type == "delete")
                {
                    if (!views.TryGetValue(patch.Id!, out var view)) continue;
                    if (view.Fields.Contains(active)) active = null;
                    Blocks.Children.Remove(view.Root);
                    views.Remove(patch.Id!);
                    blockOrder.Remove(patch.Id!);
                }
                else
                {
                    var block = patch.Block!;
                    if (!views.ContainsKey(block.Id)) blockOrder.Insert(patch.Index, block.Id);
                    Upsert(patch.Index, block, redo);
                }
            }
            ArrangeBlocks();
            revision = update.Revision;
        }
        finally { projecting = false; }
        if (update.Patches.Length > 0)
        {
            if (prototypeMode) patches.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
            else refreshTimer?.Start();
        }
        Report();
    }

    private static object Replace(Field field, int start, int end, string text) =>
        new { type = "replace", id = field.Id, cell = field.Cell, start, end, text };

    private void Apply(object operation, bool external = false)
    {
        ApplyBatch([operation], external);
    }

    private void ApplyBatch(object[] operations, bool external = false)
    {
        var expected = revision;
        if (editor is not null) Project(editor.Apply(expected, operations, external));
        else if (offline is not null) Commit(() => offline.Apply(expected, operations, external));
    }

    private void ApplyExternal(Field field, int offset)
    {
        var text = field.Content.Text;
        offset = ClampCaret(text, Math.Max(offset, 0));
        Apply(Replace(field, offset, offset, "✓ "), external: true);
    }

    private void FlushDeferred()
    {
        while (deferred.TryPeek(out var target) && !target.Field.Composing)
        {
            deferred.Dequeue();
            ApplyExternal(target.Field, target.Offset);
        }
    }

    private void SendPatch(VirtualKey key)
    {
        var fields = views.Values.SelectMany(view => view.Fields);
        var target = key switch
        {
            VirtualKey.F7 => active,
            VirtualKey.F8 when active?.Cell is { } cell =>
                views[active.Id].Fields.FirstOrDefault(field => field.Cell?.Row == cell.Row && field.Cell != cell),
            VirtualKey.F8 => null,
            _ => fields.FirstOrDefault(field => field.Id != active?.Id),
        };
        if (target is null) return;
        if (target.Composing)
        {
            deferred.Enqueue((target, target.CompositionStart));
            Report("Patch queued until IME ends.");
        }
        else ApplyExternal(target, 0);
    }

    private void UndoClick(object sender, RoutedEventArgs args) => Run(() => History(false));
    private void RedoClick(object sender, RoutedEventArgs args) => Run(() => History(true));
    private void PatchOtherClick(object sender, RoutedEventArgs args) => Run(() => SendPatch(VirtualKey.F6));
    private void PatchActiveClick(object sender, RoutedEventArgs args) => Run(() => SendPatch(VirtualKey.F7));
    private void ReplayClick(object sender, RoutedEventArgs args) => Run(Replay);

    private void Replay()
    {
        if (views.Values.SelectMany(view => view.Fields).Any(field => field.Composing)) return;
        Reset();
        var operations = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "M0", "operations-v1.json"));
        var expected = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "M0", "expected-v1.json"));
        using var sequence = JsonDocument.Parse(operations);
        foreach (var operation in sequence.RootElement.EnumerateArray())
            Project(editor!.ApplyJson($$"""{"revision":{{revision}},"operations":[{{operation.GetRawText()}}]}"""));
        using var snapshot = JsonDocument.Parse(editor!.SnapshotJson());
        using var expectedSnapshot = JsonDocument.Parse(expected);
        var projected = views.Values.SelectMany(view => view.Fields)
            .All(field => ReadText(field) == field.Content.Text);
        Report(projected && JsonElement.DeepEquals(snapshot.RootElement, expectedSnapshot.RootElement)
            ? "Fixture replay PASS." : "Fixture replay FAIL.");
    }

    private void History(bool redo)
    {
        if (IsComposing || ReadOnlyNote) return;
        var expected = revision;
        if (editor is not null) Project(editor.History(expected, redo), redo);
        else if (offline is not null && currentNote is not null)
            Commit(() => offline.History(expected, redo), redo);
    }

    private void PageKeys(object sender, KeyRoutedEventArgs args)
    {
        var key = args.Key;
        ResetAutoLock();
#if DEBUG
        var allowPatchKeys = prototypeMode || Environment.GetCommandLineArgs().Contains("--verify-ime-patches");
#else
        var allowPatchKeys = prototypeMode;
#endif
        if (allowPatchKeys && !ReadOnlyNote && key is VirtualKey.F6 or VirtualKey.F7 or VirtualKey.F8)
        {
            args.Handled = true;
            Run(() => SendPatch(key));
            return;
        }
        if (!prototypeMode && HandleOfflineKeys(args)) return;
        if (active is not { } field || field.Composing || field.Control.FocusState == FocusState.Unfocused) return;
        if (ReadOnlyNote || pending is not null) return;

        var shift = IsDown(VirtualKey.Shift);
        if (IsDown(VirtualKey.Control) && key is VirtualKey.Z or VirtualKey.Y)
        {
            args.Handled = true;
            Run(() => History(shift || key == VirtualKey.Y));
        }
        else if (field.Checklist && key == VirtualKey.Tab)
        {
            args.Handled = true;
            Run(() => Apply(new { type = "indent", id = field.Id, delta = shift ? -1 : 1 }));
        }
        else if (field.Checklist && key == VirtualKey.Enter && shift)
        {
            args.Handled = true;
            Run(() =>
            {
                var (start, end) = Selection(field);
                Apply(Replace(field, start, end, "\n"));
                field.Control.Document.Selection.SetRange(start + 1, start + 1);
            });
        }
        else if ((field.Checklist || (!prototypeMode && field.Cell is null)) && key == VirtualKey.Enter && !shift)
        {
            args.Handled = true;
            Run(() =>
            {
                var selection = field.Control.Document.Selection;
                var offset = Math.Min(selection.StartPosition, field.Content.Text.Length);
                var end = Math.Min(selection.EndPosition, field.Content.Text.Length);
                var newId = Guid.NewGuid().ToString("N");
                object split = new { type = field.Checklist ? "split_checklist" : "split_paragraph",
                    id = field.Id, offset, new_id = newId };
                var operations = offset == end ? new[] { split } : new[] { Replace(field, offset, end, ""), split };
                ApplyBatch(operations);
                var next = views[newId].Fields[0].Control;
                next.Focus(FocusState.Programmatic);
                next.Document.Selection.SetRange(0, 0);
            });
        }
        else if (!prototypeMode && key == VirtualKey.Back && field.Cell is null && Selection(field) == (0, 0))
        {
            var index = blockOrder.IndexOf(field.Id);
            if (index <= 0) return;
            var previous = views[blockOrder[index - 1]];
            if (ChecklistSort.IsChecked == true && (field.Checklist || previous.Kind == "checklist")) return;
            if (previous.Fields is not [{ Cell: null } target]) return;
            args.Handled = true;
            Run(() =>
            {
                var offset = target.Content.Text.Length;
                Apply(new { type = "merge_previous", id = field.Id });
                target.Control.Focus(FocusState.Programmatic);
                target.Control.Document.Selection.SetRange(offset, offset);
            });
        }
    }

    private static bool IsDown(VirtualKey key) =>
        (InputKeyboardSource.GetKeyStateForCurrentThread(key) & CoreVirtualKeyStates.Down) != 0;

    private void Run(Action action)
    {
        try { action(); }
        catch (Exception error) { Report(error.Message); }
    }

    private void Report(string message = "")
    {
        if (!prototypeMode)
        {
            Status.Text = message.Length > 0 ? message
                : pending is not null ? "Có thay đổi chưa lưu"
                : IsComposing ? "Đang nhập…"
                : currentNote is null ? "Chọn hoặc tạo ghi chú."
                : currentNote.Deleted ? "Ghi chú đã xóa"
                : "Đã lưu";
            return;
        }
        var composition = views.Values.SelectMany(view => view.Fields).Any(field => field.Composing)
            ? " · IME composing" : "";
        Status.Text = $"Revision {revision}{composition} · {message}\nEdit to projection: {Summary(typing)} · Patch: {Summary(patches)}";
    }

    private static string Summary(List<double> samples)
    {
        if (samples.Count == 0) return "0 samples";
        var sorted = samples.Order().ToArray();
        return $"{sorted.Length} samples, p50 {sorted[(int)Math.Ceiling(sorted.Length * .5) - 1]:F3} ms, "
            + $"p95 {sorted[(int)Math.Ceiling(sorted.Length * .95) - 1]:F3} ms";
    }
}
