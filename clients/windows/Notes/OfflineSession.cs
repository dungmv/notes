using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;

namespace Notes;

// The native session needs exclusive access. Open can run on a worker before
// ownership passes to the UI dispatcher. Calls after that stay on the UI thread.
internal sealed partial class OfflineSession : SafeHandleZeroOrMinusOneIsInvalid
{
    private const string Library = "notes_ffi.dll";
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    };

    private OfflineSession(IntPtr native) : base(ownsHandle: true) => SetHandle(native);

    internal static OfflineSession Open(string directory, string? dbKeyHex)
    {
        ReadReply(notes_offline_open(directory, dbKeyHex, out var native));
        return new OfflineSession(native);
    }

    internal T Call<T>(object request) =>
        ReadReply(notes_offline_call(this, JsonSerializer.Serialize(request, Json))).Deserialize<T>(Json)!;

    internal void Call(object request) => Call<JsonElement>(request);

    internal EditorUpdate Apply(ulong revision, object[] operations, bool external) =>
        Call<EditorUpdate>(new { type = "apply", batch = new { revision, operations, external } });

    internal EditorUpdate History(ulong revision, bool redo) =>
        Call<EditorUpdate>(new { type = "history", revision, redo });

    private static JsonElement ReadReply(IntPtr buffer)
    {
        try
        {
            using var reply = JsonDocument.Parse(Marshal.PtrToStringUTF8(buffer)
                ?? throw new InvalidOperationException("Core không trả dữ liệu."));
            if (reply.RootElement.TryGetProperty("code", out var code))
            {
                var codeStr = code.GetString();
                if (codeStr == "history_conflict") throw new HistoryConflictException();
                if (codeStr == "sync_unauthorized")
                    throw new SyncUnauthorizedException(reply.RootElement.GetProperty("error").GetString() ?? "Phiên đồng bộ hết hạn.");
                if (codeStr == "sync_network")
                    throw new SyncNetworkException(reply.RootElement.GetProperty("error").GetString() ?? "Chờ mạng.");
                if (codeStr == "note_locked") throw new NoteLockedException();
                if (codeStr == "wrong_passphrase")
                    throw new WrongPassphraseException(reply.RootElement.GetProperty("error").GetString() ?? "Mật khẩu khóa không đúng.");
                if (codeStr == "lock_unsupported")
                    throw new LockUnsupportedException(reply.RootElement.GetProperty("error").GetString() ?? "Kho dữ liệu chưa mã hóa.");
                if (codeStr == "lock_sync_enabled")
                    throw new SyncStillEnabledException(reply.RootElement.GetProperty("error").GetString() ?? "Không thể khóa ghi chú khi đồng bộ đang bật.");
            }
            if (reply.RootElement.TryGetProperty("error", out var error))
                throw new InvalidOperationException(error.GetString());
            return reply.RootElement.GetProperty("value").Clone();
        }
        finally { notes_m0_string_free(buffer); }
    }

    protected override bool ReleaseHandle()
    {
        notes_offline_free(handle);
        return true;
    }

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    private static partial IntPtr notes_offline_open(string path, string? dbKeyHex, out IntPtr session);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    private static partial IntPtr notes_offline_call(OfflineSession session, string request);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    private static partial void notes_offline_free(IntPtr session);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    private static partial void notes_m0_string_free(IntPtr buffer);
}

internal sealed class HistoryConflictException() :
    InvalidOperationException("Không thể hoàn tác hoặc làm lại vì khối đã có thay đổi mới.");

internal sealed class SyncUnauthorizedException(string message) : InvalidOperationException(message);

internal sealed class SyncNetworkException(string message) : InvalidOperationException(message);

internal sealed class NoteLockedException() :
    InvalidOperationException("Ghi chú đang khóa. Hãy mở khóa để xem nội dung.");

internal sealed class WrongPassphraseException(string message) : InvalidOperationException(message);

internal sealed class LockUnsupportedException(string message) : InvalidOperationException(message);

internal sealed class SyncStillEnabledException(string message) : InvalidOperationException(message);

internal sealed record LockStatusDto(bool Supported, string[] LockedNoteIds, bool OpenNoteLocked);

internal sealed record ImageDataDto(string Data, string MimeType);

internal sealed record SyncStatusDto(
    bool Enabled,
    bool Authenticated,
    bool Unlocked,
    string ServerUrl,
    string Email,
    string UserId,
    string DeviceId,
    uint KeyVersion,
    long LastServerCursor,
    long LastSyncedAt,
    long PendingOutboxCount,
    string? LastError);

internal sealed record AuthSuccessDto(
    string AmkB64,
    string RefreshToken,
    string? RecoveryKey,
    SyncStatusDto Status);

internal sealed record SyncRunResultDto(
    int Pushed,
    int Pulled,
    bool EmptyPull,
    ulong DurationMs,
    string[] ChangedNoteIds,
    bool FoldersChanged,
    bool NotesListChanged,
    SyncStatusDto Status);

internal sealed record DeviceInfoDto(
    string Id,
    string DeviceName,
    string Platform,
    long LastSeenAt,
    long? RevokedAt,
    long CreatedAt,
    bool IsCurrent);

internal sealed record FolderItem(string Id, string? ParentId, string Name, string Kind, JsonElement? SmartQuery)
{
    public bool IsSmartFolder => SmartQuery is not null && SmartQuery.Value.ValueKind == JsonValueKind.Object;
    public override string ToString() => Name;
}

public sealed record NoteItem(string Id, string FolderId, string Title, string Snippet, bool Deleted, bool Locked = false)
{
    public string DisplayTitle => Title.Length == 0 ? "Ghi chú chưa có tiêu đề" : Title;
    public Microsoft.UI.Xaml.Visibility LockGlyph => Locked ? Microsoft.UI.Xaml.Visibility.Visible : Microsoft.UI.Xaml.Visibility.Collapsed;
    public override string ToString() => DisplayTitle;
}

internal sealed record TextMatch(int Start, int End);
