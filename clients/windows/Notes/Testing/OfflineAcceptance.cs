#if DEBUG
using System.Text.Json;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Streams;

namespace Notes;

public sealed partial class MainPage
{
    private async Task VerifyOffline(string root)
    {
        var passed = new List<string>();
        try
        {
            Trace(root, "start");
            var folder = offline!.Call<string>(new { type = "create_folder", name = "Công việc" });
            Trace(root, "folder-created");
            RefreshFolders(folder);
            CreateNote();
            var target = currentNote ?? throw new InvalidOperationException("CreateNote did not open an editor.");
            var field = views[blockOrder[0]].Fields[0];
            const string title = "Đề xuất Tiếng Việt 😀";
            await EnterAcceptanceText(field, title);
            Require(GetNote(target.Id).Title == title && Status.Text == "Đã lưu", "Native edit and commit");
            GlobalSearch.Text = "de xuat tieng viet";
            RefreshNotes();
            Require(NoteList.Items.Cast<NoteItem>().Single().Id == target.Id, "FTS5 search from WinUI");
            FindQuery.Text = "tieng viet";
            FindNext();
            var expectedStart = title.IndexOf("Tiếng", StringComparison.Ordinal);
            Require(Selection(field) == (expectedStart, expectedStart + "Tiếng Việt".Length), "Find original UTF-16 range");
            Format("bold");
            Require(field.Content.Marks.Single().Style == "bold", "Native selection and bold");
            History(false);
            Require(field.Content.Marks.Length == 0, "Native undo");
            History(true);
            Require(field.Content.Marks.Single().Style == "bold", "Native redo");
            passed.Add("WinUI edit, ACK, Unicode search, rich text, undo and redo");
            Trace(root, "step1-text");

            var expected = revision;
            var attempts = 0;
            try
            {
                Commit(() =>
                {
                    if (attempts++ == 0) throw new IOException("Simulated write failure");
                    return offline.Apply(expected, [Replace(field, title.Length, title.Length, "!")], false);
                });
            }
            catch (InvalidOperationException error) when (error.InnerException is IOException) { }
            Require(pending is not null && field.Control.IsReadOnly && !TryCloseEditor() &&
                GetNote(target.Id).Title == title, "Preserve the pending edit and block navigation on write failure");
            RetryClick(RetryButton, new RoutedEventArgs());
            Require(pending is null && revision == expected + 1 && !field.Control.IsReadOnly &&
                GetNote(target.Id).Title == title + "!", "Retry the pending write on its original note once");
            passed.Add("WinUI pending state, navigation guard and retry after an injected error");
            Trace(root, "step2-retry");

            CreateNote();
            var source = currentNote ?? throw new InvalidOperationException("Source note missing.");
            field = views[blockOrder[0]].Fields[0];
            await EnterAcceptanceText(field, "Nguồn");
            field.Control.Document.Selection.SetRange(5, 5);
            InsertNoteLink(field, 5, 5, target, null);
            var tail = views[blockOrder[^1]].Fields[0];
            InsertNoteLink(tail, 0, 0, target, "Nhãn riêng");
            OpenLinkedNote(target.Id);
            Require(currentNote?.Id == target.Id, "Open a link by stable note ID");
            await EnterAcceptanceText(views[blockOrder[0]].Fields[0], "Đích đã đổi tên");
            offline.Call(new { type = "move_note", id = target.Id, folder = "inbox" });
            SetCurrent(GetNote(source.Id));
            Require(LinkLabels().SequenceEqual(new[] { "Đích đã đổi tên", "Nhãn riêng" }),
                "Refresh the title label while retaining the custom label");
            passed.Add("Link insertion, navigation, rename, move and both label modes");
            Trace(root, "step3-links");

            CreateNote();
            var checklist = currentNote ?? throw new InvalidOperationException("Checklist note missing.");
            await EnterAcceptanceText(views[blockOrder[0]].Fields[0], "[] ");
            var firstId = blockOrder[0];
            Require(views[firstId].Toggle is not null, "Automatic checklist conversion");
            await EnterAcceptanceText(views[firstId].Fields[0], "Mục đầu");
            Apply(new { type = "split_checklist", id = firstId, offset = "Mục đầu".Length, new_id = "acceptance-tail" });
            Apply(new { type = "indent", id = "acceptance-tail", delta = 1 });
            await EnterAcceptanceText(views["acceptance-tail"].Fields[0], "Mục sau");
            ChecklistSort.IsChecked = true;
            ChecklistSortClick(ChecklistSort, new RoutedEventArgs());
            Apply(new { type = "toggle", id = firstId });
            Require(Blocks.Children[1] == views[firstId].Root &&
                views["acceptance-tail"].Root.Margin.Left > 0, "Checklist sorts checked items and preserves depth");
            Apply(new { type = "toggle", id = firstId });
            Require(Blocks.Children[0] == views[firstId].Root, "Unchecked checklist item returns to its group");
            passed.Add("Checklist conversion, split, indent, sort and toggle");
            Trace(root, "step4-checklist");

            CreateNote();
            var photo = currentNote ?? throw new InvalidOperationException("Photo note missing.");
            field = views[blockOrder[0]].Fields[0];
            await EnterAcceptanceText(field, "Tiếng Việt 😀");
            field.Control.Document.Selection.SetRange(5, 5);
            Trace(root, "img-before-storagefile");
            var fixture = await StorageFile.GetFileFromPathAsync(Path.Combine(AppContext.BaseDirectory, "M1", "pixel.png"));
            Trace(root, "img-after-storagefile");
            var clipboard = new DataPackage();
            clipboard.SetBitmap(RandomAccessStreamReference.CreateFromFile(fixture));
            Trace(root, "img-before-clipboard");
            await InsertClipboardImage(field, clipboard.GetView());
            Trace(root, "img-after-clipboard");
            Require(blockOrder.Count == 3, "Insert bitmap between two text blocks");
            SetCurrent(null);
            offline.Dispose();
            Trace(root, "img-before-reopen");
            offline = OfflineSession.Open(root, LocalDbKeyStore.LoadOrCreate(root));
            Trace(root, "img-after-reopen");
            RefreshFolders("all");
            SetCurrent(GetNote(photo.Id));
            var image = ((Panel)views[blockOrder[1]].Root).Children.OfType<Image>().Single();
            for (var attempt = 0; image.Source is null && attempt < 100; attempt++) await Task.Delay(25);
            Require(image.Source is not null && ReadText(views[blockOrder[0]].Fields[0]) == "Tiếng" &&
                ReadText(views[blockOrder[2]].Fields[0]) == " Việt 😀", "Reload bitmap from SQLite metadata and blob");
            SetCurrent(GetNote(checklist.Id));
            Require(views[firstId].Toggle?.IsChecked == false &&
                views["acceptance-tail"].Root.Margin.Left > 0 &&
                offline.Call<bool>(new { type = "checklist_sort" }), "Reload persisted checklist items and setting");
            passed.Add("Bitmap DataPackage, PNG encoding, caret position, blob rendering and reopen");
            Trace(root, "step5-image");

            SetCurrent(GetNote(target.Id));
            DeleteClick(DeleteButton, new RoutedEventArgs());
            RefreshFolders("trash");
            SetCurrent(GetNote(target.Id));
            Require(currentNote?.Deleted == true && ReadOnlyNote, "WinUI trash and read-only rendering");
            DeleteClick(DeleteButton, new RoutedEventArgs());
            Require(GetNote(target.Id).Deleted == false && GetNote(target.Id).FolderId == "inbox",
                "Restore note to previous folder");
            passed.Add("Soft deletion and restoration on the same stable ID");
            Trace(root, "step6-trash");

            WriteAcceptanceResult(root, passed, null);
        }
        catch (Exception error)
        {
            WriteAcceptanceResult(root, passed, error.ToString());
        }
        finally
        {
            CloseSession();
            Application.Current.Exit();
        }
    }

    private async Task EnterAcceptanceText(Field field, string text, bool expectFailure = false)
    {
        for (var attempt = 0; !field.Control.IsLoaded && attempt < 100; attempt++) await Task.Delay(25);
        Require(field.Control.IsLoaded, "RichEditBox loaded");
        var previous = revision;
        field.Control.Focus(FocusState.Programmatic);
        field.Control.Document.SetText(TextSetOptions.None, text);
        // RichEditBox raises TextChanged through the XAML dispatcher.
        for (var attempt = 0; revision == previous && pending is null && attempt < 100; attempt++) await Task.Delay(25);
        Require(expectFailure ? pending is not null && revision == previous : revision > previous,
            $"Unexpected RichEditBox write result. State: {Status.Text}.");
    }

    private IEnumerable<string> LinkLabels() => blockOrder
        .Where(id => views[id].Kind == "note_link")
        .Select(id => (string)((Panel)views[id].Root).Children.OfType<Button>().First().Content);

    private static void Require(bool condition, string expectation)
    {
        if (!condition) throw new InvalidOperationException(expectation);
    }

    private static void WriteAcceptanceResult(string root, List<string> passed, string? error)
    {
        Trace(root, $"result error={error ?? "none"} passed={passed.Count}");
        File.WriteAllText(Path.Combine(root, "winui-acceptance.json"),
            JsonSerializer.Serialize(new { passed, error }, new JsonSerializerOptions { WriteIndented = true }));
    }

    private static void Trace(string root, string message)
    {
        try { File.AppendAllText(Path.Combine(root, "verify-trace.log"),
            $"{DateTime.Now:HH:mm:ss.fff} {message}\n"); }
        catch { }
    }
}
#endif
