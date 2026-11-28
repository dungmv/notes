#if DEBUG
using System.Security.Cryptography;
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
    private const string FixtureNote = "desktop-v1";
    private const string FixtureImageHash = "42ff80f86ca28d848679ebfe6118a085c5d1ea9270ee3075be205e0d22bf673b";
    private List<double>? acceptanceAcks;
    private List<double>? acceptanceProjections;

    private async Task VerifyM2(string root, string mode)
    {
        var passed = new List<string>();
        object? measurements = null;
        string? error = null;
        try
        {
            var ackFile = Path.Combine(root, "ack.json");
            if (mode == "--m2-reopen")
            {
                await VerifyDesktopSnapshot(ackFile);
                Require(offline!.Call<NoteItem[]>(new { type = "find", query = "tiếng việt" })
                    .Any(item => item.Id == FixtureNote), "FTS after restart");
                passed.Add("WinUI reopened the ACKed snapshot, images, links, and FTS after forced exit");
                return;
            }

            var fixture = Path.Combine(root, "expected-v1.json");
            if (mode == "--m2-startup")
            {
                SetCurrent(GetNote(FixtureNote));
                var field = views["intro"].Fields[0];
                var inputTicks = 0L;
                field.Control.TextChanging += (_, _) =>
                {
                    if (inputTicks == 0) inputTicks = DateTime.UtcNow.Ticks;
                };
                await EnterAcceptanceText(field, "Tiếng Việt!");
                Require(inputTicks > 0 && field.Control.FocusState != FocusState.Unfocused,
                    "Focused native editor received the first input");
                var launchTicks = long.Parse(Environment.GetEnvironmentVariable("NOTES_M2_LAUNCH_TICKS")!);
                measurements = new { launchToInputMs = TimeSpan.FromTicks(inputTicks - launchTicks).TotalMilliseconds };
                passed.Add("New WinUI process reached focused editor and accepted the first scripted edit");
                return;
            }
            await VerifyDesktopSnapshot(fixture);
            Require(LinkLabels().SequenceEqual(new[] { "Đích fixture", "Nhãn riêng" }), "Fixture link labels");
            Require(offline!.Call<NoteItem[]>(new { type = "find", query = "tieng viet" }).Single().Id == FixtureNote,
                "Accentless FTS, including after schema 1 migration");
            var intro = views["intro"].Fields[0];
            var table = views["table"].Fields.Single(field => field.Cell == new Cell(1, 1));
            if (mode is "--m2-text" or "--m2-image")
            {
                if (mode == "--m2-text") await EnterAcceptanceText(table, "Hai 😀");
                else await PasteFixtureBitmap(intro);
                Require(Status.Text == "Đã lưu", "Native editor reported durable ACK");
                WriteCheckpoint(root, ackFile);
                // The runner terminates this process after seeing the ACK file.
                await Task.Delay(TimeSpan.FromMinutes(2));
                throw new TimeoutException("Expected the runner to terminate WinUI.");
            }

            if (mode == "--m2-retry")
            {
                var before = revision;
                var rejected = Path.Combine(root, "rejected");
                await EnterAcceptanceText(table, "Hai 😀", expectFailure: true);
                Require(pending is not null && !TryCloseEditor() && revision == before &&
                    GetNote(FixtureNote).Title == "Tiếng Việt" &&
                    offline.Call<NoteItem[]>(new { type = "find", query = "hai" }).Length == 0 &&
                    Status.Text != "Đã lưu", "SQLite rejected text without ACK or FTS update");
                File.WriteAllText(rejected, "text");
                await WaitForAcceptanceFile(Path.Combine(root, "retry-text"));
                RetryClick(RetryButton, new RoutedEventArgs());
                Require(pending is null && revision == before + 1 && Status.Text == "Đã lưu" &&
                    offline.Call<NoteItem[]>(new { type = "find", query = "hai" }).Single().Id == FixtureNote,
                    "Text retry committed and indexed once");

                try { await PasteFixtureBitmap(intro); }
                catch (InvalidOperationException) when (pending is not null) { }
                Require(pending is not null && revision == before + 1 &&
                    views.Values.Count(view => view.Kind == "image") == 1, "SQLite rejected image without ACK");
                File.WriteAllText(Path.Combine(root, "rejected-image"), "image");
                await WaitForAcceptanceFile(Path.Combine(root, "retry-image"));
                RetryClick(RetryButton, new RoutedEventArgs());
                Require(pending is null && revision == before + 2 &&
                    views.Values.Count(view => view.Kind == "image") == 2 && Status.Text == "Đã lưu",
                    "Image retry committed one reference");
                WriteCheckpoint(root, ackFile);
                await VerifyDesktopSnapshot(ackFile);
                passed.Add("Real SQLite text and image failures, close guard, retry, FTS and reopen");
                return;
            }

            Require(mode == "--m2-verify", "Unknown M2 acceptance mode");
            table.Control.UpdateLayout();
            Require(table.Control.Focus(FocusState.Programmatic) && table.Control.FocusState != FocusState.Unfocused,
                "Table cell accepts focus");
            await EnterAcceptanceText(table, "Mộta");
            table.Control.Document.Selection.SetRange(0, 3);
            Format("italic");
            History(false);
            Require(table.Content.Marks.Length == 0, "Undo table formatting");
            History(true);
            Require(table.Content.Marks.Single() == new Mark(0, 3, "italic"), "Redo table formatting");
            var unchanged = views["table"].Fields.Where(field => field != table).Select(ReadText);
            Require(unchanged.SequenceEqual(new[] { "Mục", "Giá trị", "Ghi chú" }), "Other table cells stayed intact");
            ApplyExternal(table, 0);
            try
            {
                History(false);
                throw new Exception("Undo should detect the external change to the table block.");
            }
            catch (HistoryConflictException) { }
            Require(pending is null && !table.Control.IsReadOnly && table.Content.Text == "✓ Mộta",
                "Rejected undo preserves the patch and leaves the editor usable");
            var emoji = views["link-custom-tail"].Fields[0];
            Require(emoji.Control.Focus(FocusState.Programmatic), "Focus emoji paragraph");
            for (var attempt = 0; active != emoji && attempt < 100; attempt++) await Task.Delay(25);
            Require(active == emoji, "RichEditBox dispatched emoji paragraph focus");
            emoji.Control.Document.Selection.SetRange(1, 8);
            Require(Selection(emoji) == (1, 8), "WinUI selected the emoji grapheme");
            var target = offline.Call<NoteItem[]>(new { type = "find", query = "dich fixture" }).Single();
            SetCurrent(target);
            SetCurrent(GetNote(FixtureNote));
            var restored = views[emoji.Id].Fields[0];
            Require(Selection(restored) == (1, 8),
                $"Changing notes restores the emoji selection, observed {Selection(restored)}");
            VerifyContract(root);
            passed.Add("Schema fixture, UTF-16 rejection, C# lifecycle, selection on note switch, table edits and history conflict");

            var longText = string.Concat(Enumerable.Repeat("Đề xuất cho dự án tiếng Việt và thử nghiệm 😀\n", 2_000));
            var longField = views["link-custom-tail"].Fields[0];
            await EnterAcceptanceText(longField, longText);
            Require(longField.Content.Text == longText, "RichEditBox retained the complete long fixture");
            acceptanceAcks = [];
            acceptanceProjections = [];
            for (var i = 0; i < 40; i++)
            {
                await Task.Yield();
                var previous = revision;
                longField.Control.Document.GetRange(0, 0).SetText(TextSetOptions.None, "a");
                for (var j = 0; revision == previous && pending is null && j < 200; j++) await Task.Delay(25);
                if (revision != previous + 1)
                    throw new InvalidOperationException(
                        $"Long note edit {i + 1}: expected revision {previous + 1}, observed {revision}. " +
                        $"UI {ReadText(longField).Length} UTF-16 units, core {longField.Content.Text.Length}. {Status.Text}");
            }
            Require(acceptanceAcks.Count == 40 && acceptanceProjections.Count == 40, "All timing samples recorded");
            measurements = new
            {
                longNoteBytes = System.Text.Encoding.UTF8.GetByteCount(longText),
                ackMs = acceptanceAcks,
                projectionMs = acceptanceProjections,
            };
            acceptanceAcks = acceptanceProjections = null;
            WriteCheckpoint(root, ackFile);
            await VerifyDesktopSnapshot(ackFile);
            passed.Add("40 long-note edits through RichEditBox, ACK timings, and persisted snapshot");
        }
        catch (Exception failure) { error = failure.ToString(); }
        finally
        {
            File.WriteAllText(Path.Combine(root, "m2-result.json"),
                JsonSerializer.Serialize(new { mode, passed, measurements, error },
                    new JsonSerializerOptions { WriteIndented = true }));
            CloseSession();
            Application.Current.Exit();
        }
    }

    private void VerifyContract(string root)
    {
        for (var attempt = 0; attempt < 8; attempt++)
        {
            using var session = OfflineSession.Open(root);
            var snapshot = session.Call<EditorSnapshot>(new { type = "open_note", id = FixtureNote });
            var tail = snapshot.Blocks.Single(block => block.Id == "link-custom-tail");
            var text = tail.Content!.Text;
            var insideSurrogate = Enumerable.Range(0, text.Length).First(index => char.IsLowSurrogate(text[index]));
            try
            {
                session.Apply(snapshot.Revision, [new { type = "replace", id = tail.Id,
                    start = insideSurrogate, end = insideSurrogate, text = "!" }], false);
                throw new Exception("C ABI accepted a split surrogate.");
            }
            catch (InvalidOperationException) { }
            Require(session.Call<EditorSnapshot>(new { type = "open_note", id = FixtureNote }).Revision ==
                snapshot.Revision, "Rejected UTF-16 range kept revision");
        }
    }

    private async Task VerifyDesktopSnapshot(string expectedPath)
    {
        SetCurrent(null);
        SetCurrent(GetNote(FixtureNote));
        using var expected = JsonDocument.Parse(File.ReadAllText(expectedPath));
        using var reader = OfflineSession.Open(Path.GetDirectoryName(expectedPath)!);
        var snapshot = reader.Call<JsonElement>(new { type = "open_note", id = FixtureNote });
        Require(JsonElement.DeepEquals(snapshot, expected.RootElement), "Persisted snapshot matches checkpoint");
        var images = snapshot.GetProperty("blocks").EnumerateArray()
            .Where(block => block.GetProperty("kind").GetString() == "image").ToArray();
        for (var index = 0; index < images.Length; index++)
        {
            var id = images[index].GetProperty("attachment_id").GetString();
            var path = reader.Call<string>(new { type = "image_path", id });
            var hash = Convert.ToHexString(SHA256.HashData(await File.ReadAllBytesAsync(path)));
            Require(hash.Equals(Path.GetFileName(path), StringComparison.OrdinalIgnoreCase), "Blob matches content address");
            if (index == images.Length - 1)
                Require(hash.Equals(FixtureImageHash, StringComparison.OrdinalIgnoreCase), "Original fixture blob hash");
        }
        var rendered = views.Values.Where(view => view.Kind == "image")
            .Select(view => ((Panel)view.Root).Children.OfType<Image>().Single()).ToArray();
        for (var i = 0; rendered.Any(image => image.Source is null) && i < 100; i++) await Task.Delay(25);
        Require(rendered.Length == images.Length && rendered.All(image => image.Source is not null),
            "All persisted images render");
        Require(views.Values.SelectMany(view => view.Fields).All(field => ReadText(field) == field.Content.Text),
            "Native RichEditBox text matches all core fields");
    }

    private void WriteCheckpoint(string root, string path)
    {
        using var reader = OfflineSession.Open(root);
        var snapshot = reader.Call<JsonElement>(new { type = "open_note", id = FixtureNote });
        Require(snapshot.GetProperty("revision").GetUInt64() == revision, "Durable revision equals UI ACK");
        var temporary = path + ".tmp";
        File.WriteAllText(temporary, snapshot.GetRawText());
        File.Move(temporary, path, overwrite: true);
    }

    private async Task PasteFixtureBitmap(Field field)
    {
        field.Control.Document.Selection.SetRange(5, 5);
        var file = await StorageFile.GetFileFromPathAsync(Path.Combine(AppContext.BaseDirectory, "M1", "pixel.png"));
        var clipboard = new DataPackage();
        clipboard.SetBitmap(RandomAccessStreamReference.CreateFromFile(file));
        await InsertClipboardImage(field, clipboard.GetView());
    }

    private static async Task WaitForAcceptanceFile(string path)
    {
        for (var i = 0; i < 600 && !File.Exists(path); i++) await Task.Delay(50);
        Require(File.Exists(path), $"Runner did not signal {Path.GetFileName(path)}");
    }
}
#endif
