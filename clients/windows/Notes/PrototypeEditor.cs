using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;

namespace Notes;

internal sealed class PrototypeEditor : SafeHandleZeroOrMinusOneIsInvalid
{
    private const string Library = "notes_ffi.dll";
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    };

    private PrototypeEditor() : base(ownsHandle: true) { }

    internal static PrototypeEditor Open()
    {
        var editor = notes_m0_editor_open_fixture();
        if (!editor.IsInvalid) return editor;
        editor.Dispose();
        throw new InvalidOperationException("Rust could not open the M0 fixture.");
    }

    internal string SnapshotJson() => ReadReply(notes_m0_editor_snapshot(this));

    internal EditorSnapshot Snapshot() => JsonSerializer.Deserialize<EditorSnapshot>(SnapshotJson(), Json)!;

    internal EditorUpdate Apply(ulong revision, object[] operations, bool external) =>
        ApplyJson(JsonSerializer.Serialize(new { revision, operations, external }, Json));

    internal EditorUpdate ApplyJson(string request) =>
        JsonSerializer.Deserialize<EditorUpdate>(ReadReply(notes_m0_editor_apply(this, request)), Json)!;

    internal EditorUpdate History(ulong revision, bool redo) =>
        JsonSerializer.Deserialize<EditorUpdate>(
            ReadReply(notes_m0_editor_history(this, revision, redo ? (byte)1 : (byte)0)), Json)!;

    private static string ReadReply(IntPtr buffer)
    {
        try
        {
            using var reply = JsonDocument.Parse(Marshal.PtrToStringUTF8(buffer)
                ?? throw new InvalidOperationException("Empty Rust reply."));
            if (reply.RootElement.TryGetProperty("error", out var error))
                throw new InvalidOperationException(error.GetString());
            return reply.RootElement.GetProperty("value").GetString()!;
        }
        finally
        {
            notes_m0_string_free(buffer);
        }
    }

    protected override bool ReleaseHandle()
    {
        notes_m0_editor_free(handle);
        return true;
    }

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern PrototypeEditor notes_m0_editor_open_fixture();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr notes_m0_editor_snapshot(PrototypeEditor editor);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr notes_m0_editor_apply(PrototypeEditor editor,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string request);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr notes_m0_editor_history(PrototypeEditor editor, ulong revision, byte redo);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern void notes_m0_editor_free(IntPtr editor);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern void notes_m0_string_free(IntPtr buffer);
}

internal sealed record Mark(int Start, int End, string Style);
internal sealed record RichText(string Text, Mark[] Marks);
internal sealed record Cell(int Row, int Column);
internal sealed record Block(string Id, string Kind, RichText? Content, int Depth, bool Checked, RichText[][]? Rows,
    string? Style, string? AttachmentId, string? NoteId, string? Label);
internal sealed record EditorSnapshot(ulong Revision, Block[] Blocks);
internal sealed record EditorPatch(string Type, string? Id, int Index, Block? Block);
internal sealed record EditorUpdate(ulong BaseRevision, ulong Revision, EditorPatch[] Patches);
