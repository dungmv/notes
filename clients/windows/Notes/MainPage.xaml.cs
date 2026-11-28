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
    private sealed record View(FrameworkElement Root, CheckBox? Toggle, Field[] Fields);

    private sealed class Field(string id, Cell? cell)
    {
        internal string Id { get; } = id;
        internal Cell? Cell { get; } = cell;
        internal RichEditBox Control { get; } = new() { MinHeight = 42, AcceptsReturn = true };
        internal RichText Content { get; set; } = new("", []);
        internal bool Composing { get; set; }
        internal int CompositionStart { get; set; }
        internal bool Checklist { get; init; }
        internal long Started { get; set; }
    }

    private readonly Dictionary<string, View> views = [];
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
        Loaded += (_, _) => Run(Reset);
        Unloaded += (_, _) =>
        {
            editor?.Dispose();
            editor = null;
        };
        PreviewKeyDown += PageKeys;
        Status.Text = "Opening fixture…";
    }

    private void Reset()
    {
        editor?.Dispose();
        editor = PrototypeEditor.Open();
        var snapshot = editor.Snapshot();
        revision = snapshot.Revision;
        projecting = true;
        try
        {
            deferred.Clear();
            active = null;
            typing.Clear();
            patches.Clear();
            views.Clear();
            Blocks.Children.Clear();
            foreach (var block in snapshot.Blocks) Upsert(Blocks.Children.Count, block);
        }
        finally { projecting = false; }
        Report();
    }

    private void Upsert(int index, Block block, bool redo = false)
    {
        if (!views.TryGetValue(block.Id, out var view))
        {
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
                    toggle = new CheckBox { MinWidth = 0 };
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
            view = new(grid, toggle, [.. fields]);
            views.Add(block.Id, view);
            Blocks.Children.Insert(index, grid);
        }

        view.Root.Margin = new Thickness(block.Depth * 24, 0, 0, 0);
        if (view.Toggle is { } box) box.IsChecked = block.Checked;
        foreach (var field in view.Fields)
        {
            var content = field.Cell is { } cell ? block.Rows![cell.Row][cell.Column] : block.Content!;
            if (field.Content.Text != content.Text || !field.Content.Marks.SequenceEqual(content.Marks))
                Project(field, content, redo && field == active);
        }
    }

    private Field NewField(string id, Cell? cell, bool checklist)
    {
        var field = new Field(id, cell) { Checklist = checklist };
        var box = field.Control;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, cell is null ? id : $"{id} {cell.Row} {cell.Column}");
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
        };
        return field;
    }

    private void SubmitText(Field field)
    {
        var started = field.Started;
        field.Started = 0;
        var current = ReadText(field);
        var old = field.Content.Text;
        if (current == old || editor is null) return;
        var change = Difference(old, current);
        Apply(Replace(field, change.Start, change.End, change.Text));
        if (started != 0) typing.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
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
        foreach (var mark in content.Marks)
        {
            var marked = doc.GetRange(mark.Start, mark.End).CharacterFormat;
            if (mark.Style == "bold") marked.Bold = FormatEffect.On;
            if (mark.Style == "italic") marked.Italic = FormatEffect.On;
        }
        selection.SetRange(Math.Clamp(anchor, 0, content.Text.Length), Math.Clamp(end, 0, content.Text.Length));
        field.Content = content;
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
                    var view = views[patch.Id!];
                    if (view.Fields.Contains(active)) active = null;
                    Blocks.Children.Remove(view.Root);
                    views.Remove(patch.Id!);
                }
                else Upsert(patch.Index, patch.Block!, redo);
            }
            revision = update.Revision;
        }
        finally { projecting = false; }
        if (update.Patches.Length > 0) patches.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
        Report();
    }

    private static object Replace(Field field, int start, int end, string text) =>
        new { type = "replace", id = field.Id, cell = field.Cell, start, end, text };

    private void Apply(object operation, bool external = false)
    {
        if (editor is not null) Project(editor.Apply(revision, [operation], external));
    }

    private void ApplyExternal(Field field, int offset)
    {
        var text = field.Content.Text;
        offset = Math.Clamp(offset, 0, text.Length);
        if (offset < text.Length && char.IsLowSurrogate(text[offset])) offset--;
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
        if (editor is not null && !views.Values.SelectMany(view => view.Fields).Any(field => field.Composing))
            Project(editor.History(revision, redo), redo);
    }

    private void PageKeys(object sender, KeyRoutedEventArgs args)
    {
        var key = args.Key;
        if (key is VirtualKey.F6 or VirtualKey.F7 or VirtualKey.F8)
        {
            args.Handled = true;
            Run(() => SendPatch(key));
            return;
        }
        if (active is not { } field || field.Composing || field.Control.FocusState == FocusState.Unfocused) return;

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
        else if (field.Checklist && key == VirtualKey.Enter && !shift)
        {
            args.Handled = true;
            Run(() =>
            {
                var selection = field.Control.Document.Selection;
                var offset = Math.Min(selection.StartPosition, field.Content.Text.Length);
                var end = Math.Min(selection.EndPosition, field.Content.Text.Length);
                var newId = Guid.NewGuid().ToString("N");
                object split = new { type = "split_checklist", id = field.Id, offset, new_id = newId };
                var operations = offset == end ? new[] { split } : new[] { Replace(field, offset, end, ""), split };
                Project(editor!.Apply(revision, operations, false));
                var next = views[newId].Fields[0].Control;
                next.Focus(FocusState.Programmatic);
                next.Document.Selection.SetRange(0, 0);
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
