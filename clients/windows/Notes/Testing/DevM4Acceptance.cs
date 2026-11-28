#if DEBUG
using System.Text.Json;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.System;

namespace Notes;

// Harness nghiệm thu M4 bản dev: bảng, tag, Smart Folder, Locked Notes và
// import/export chạy qua FFI thật trên bản build Debug, kết quả ghi JSON.
public sealed partial class MainPage
{
    private async Task VerifyM4(string root)
    {
        var passed = new List<string>();
        try
        {
            TraceM4(root, "start");

            // 1. Bảng: chèn, thêm hàng, sửa ô, di chuyển, xóa hàng.
            CreateNote();
            var note = currentNote ?? throw new InvalidOperationException("note missing");
            var field = views[blockOrder[0]].Fields[0];
            await EnterAcceptanceText(field, "M4 table note #kiem-tra-m4");
            var table = new
            {
                id = "m4-table",
                kind = "table",
                rows = new[]
                {
                    new[] { CellText(""), CellText("") },
                    new[] { CellText(""), CellText("") },
                },
            };
            Apply(new { type = "insert_at", id = field.Id,
                start = "M4 table note #kiem-tra-m4".Length, end = "M4 table note #kiem-tra-m4".Length,
                block = table, tail_id = "m4-tail" });
            Require(views.ContainsKey("m4-table") && views["m4-table"].Fields.Length == 4,
                "Table 2x2 inserted");
            Apply(new { type = "table_insert_row", id = "m4-table", after_row = 0 });
            Require(views["m4-table"].Fields.Length == 6, "Row inserted (3x2)");
            Apply(new { type = "replace", id = "m4-table",
                cell = new { row = 1, column = 1 }, start = 0, end = 0, text = "bí mật ô bảng" });
            Apply(new { type = "table_move_row", id = "m4-table", from = 0, to = 2 });
            Apply(new { type = "table_remove_row", id = "m4-table", row = 2 });
            Require(views["m4-table"].Fields.Length == 4, "Row moved and removed (2x2)");
            var tableText = views["m4-table"].Fields
                .FirstOrDefault(f => f.Cell?.Row == 0 && f.Cell?.Column == 1);
            Require(tableText is not null && ReadText(tableText) == "bí mật ô bảng",
                "Cell content follows structural change");
            passed.Add("Table insert, row ops, cell edit and move");

            // 2. Tag inline + Tag Browser.
            var tags = offline!.Call<TagCountDto[]>(new { type = "tags_list" });
            Require(tags.Any(tag => tag.Tag == "kiem-tra-m4"), "Tag extracted to note_tags");
            RefreshTags();
            Require(TagList.Items.Count > 0, "Tag browser populated");
            Require(offline.Call<NoteItem[]>(new { type = "find", query = "#kiem-tra-m4" })
                .Any(item => item.Id == note.Id), "Find by #tag");
            passed.Add("Inline tag, tag browser and #tag search");

            // 3. Smart Folder.
            var smart = offline.Call<string>(new
            {
                type = "create_smart_folder",
                name = "M4 Smart",
                query = "{\"tag\":\"kiem-tra-m4\"}",
            });
            Require(offline.Call<NoteItem[]>(new { type = "smart_folder_notes", id = smart })
                .Any(item => item.Id == note.Id), "Smart folder matches tagged note");
            var invalid = false;
            try
            {
                offline.Call<string>(new { type = "create_smart_folder",
                    name = "x", query = "{\"tag\":123}" });
            }
            catch (InvalidOperationException) { invalid = true; }
            Require(invalid, "Invalid smart query rejected");
            passed.Add("Smart Folder create, evaluate and reject invalid DSL");
            GlobalSearch.Text = "";
            RefreshFolders("all");
            RefreshNotes();
            SetCurrent(null);
            SetCurrent(GetNote(note.Id));

            // 4. Locked Notes: khóa, scrub, mở bằng UI, khóa lại, gỡ bảo vệ.
            offline.Call(new { type = "lock_note", id = note.Id, passphrase = "m4-pass" });
            var locked = GetNote(note.Id);
            Require(locked.Locked && locked.Snippet.Length == 0, "Locked note scrubbed");
            Require(!offline.Call<NoteItem[]>(new { type = "find", query = "bi mat o bang" })
                .Any(item => item.Id == note.Id), "Locked table cell hidden from FTS");
            try
            {
                offline.Call<EditorSnapshot>(new { type = "open_note", id = note.Id });
                throw new Exception("open_note should fail on locked note");
            }
            catch (NoteLockedException) { }
            SetCurrent(locked);
            Require(LockScreen.Visibility == Visibility.Visible, "Lock screen shown");
            Require(LockPassphrase.IsEnabled, "Passphrase box ready");
            await UnlockWith("m4-pass");
            Require(IsLockedSession && LockScreen.Visibility == Visibility.Collapsed,
                "Unlocked session restores editor");
            Require(ReadText(views["m4-table"].Fields
                .First(f => f.Cell?.Row == 0 && f.Cell?.Column == 1)) == "bí mật ô bảng",
                "Unlocked table content readable");
            RelockClick(RelockButton, new RoutedEventArgs());
            Require(GetNote(note.Id).Locked, "Relocked from UI");
            SetCurrent(GetNote(note.Id));
            await UnlockWith("m4-pass");
            offline.Call(new { type = "lock_remove_protection", id = note.Id, passphrase = "m4-pass" });
            Require(!GetNote(note.Id).Locked, "Protection removed");
            Require(offline.Call<NoteItem[]>(new { type = "find", query = "bi mat o bang" })
                .Any(item => item.Id == note.Id), "FTS restored after protection removal");
            passed.Add("Locked Notes lock, scrub, unlock via lock screen, relock, remove protection");

            // 5. Export/Import Markdown round trip (có ảnh).
            var snapshot = offline.Call<EditorSnapshot>(new { type = "open_note", id = note.Id });
            var lastBlock = snapshot.Blocks[^1].Id;
            var png = File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "M1", "pixel.png"));
            offline.Call<EditorUpdate>(new
            {
                type = "paste_image", revision = snapshot.Revision, id = lastBlock,
                start = 0, end = 0, png = Convert.ToBase64String(png),
            });
            var exportDir = Path.Combine(root, "export-m4");
            var path = offline.Call<string>(new
            {
                type = "export_note", id = note.Id, format = "markdown", dir = exportDir,
            });
            Require(File.Exists(path), "Markdown export written");
            Require(Directory.Exists(Path.Combine(exportDir, "attachments")), "Attachments folder exported");
            var imported = offline.Call<string[]>(new
            {
                type = "import_folder", path = exportDir, parent = (string?)null, format = "markdown",
            });
            Require(imported.Length == 1, "Import one note from markdown folder");
            var importedSnapshot = offline.Call<EditorSnapshot>(new { type = "open_note", id = imported[0] });
            Require(importedSnapshot.Blocks.Any(block => block.Kind == "table"),
                "Imported note keeps the table");
            Require(importedSnapshot.Blocks.Any(block => block.Kind == "image"),
                "Imported note keeps the image");
            passed.Add("Markdown export/import round trip with table and image folder");

            // 6. Gallery toggle.
            ViewToggleClick(ViewToggle, new RoutedEventArgs());
            Require(GalleryList.Visibility == Visibility.Visible &&
                NoteList.Visibility == Visibility.Collapsed, "Gallery view shown");
            Require(GalleryList.ItemsSource is IEnumerable<NoteItem> cards && cards.Any(),
                "Gallery cards filled");
            ViewToggleClick(ViewToggle, new RoutedEventArgs());
            Require(NoteList.Visibility == Visibility.Visible, "List view restored");
            passed.Add("Gallery/List view toggle");

            WriteM4Result(root, passed, null);
        }
        catch (Exception error)
        {
            WriteM4Result(root, passed, error.ToString());
        }
        finally
        {
            CloseSession();
            Application.Current.Exit();
        }
    }

    private static object CellText(string text) =>
        new { text, marks = Array.Empty<object>() };

    private static void WriteM4Result(string root, List<string> passed, string? error)
    {
        TraceM4(root, $"result error={error ?? "none"} passed={passed.Count}");
        File.WriteAllText(Path.Combine(root, "m4-dev-acceptance.json"),
            JsonSerializer.Serialize(new { passed, error },
                new JsonSerializerOptions { WriteIndented = true }));
    }

    private static void TraceM4(string root, string message)
    {
        try
        {
            File.AppendAllText(Path.Combine(root, "m4-dev-trace.log"),
                $"{DateTime.Now:HH:mm:ss.fff} {message}\n");
        }
        catch { }
    }
}
#endif
