using System.Runtime.InteropServices.WindowsRuntime;
using System.Text;
using System.Text.Json;
using Windows.Security.Cryptography.DataProtection;

namespace Notes;

internal sealed record SavedSyncSecrets(string AmkB64, string RefreshToken);

internal static class SecureKeyStore
{
    private const string FileName = "sync-session.dpapi";

    internal static async Task SaveAsync(string root, string amkB64, string refreshToken)
    {
        var json = JsonSerializer.Serialize(new SavedSyncSecrets(amkB64, refreshToken));
        var plain = Encoding.UTF8.GetBytes(json);
        var provider = new DataProtectionProvider("LOCAL=user");
        var protectedBuffer = await provider.ProtectAsync(plain.AsBuffer());
        Array.Clear(plain, 0, plain.Length);
        var path = Path.Combine(root, FileName);
        await File.WriteAllBytesAsync(path, protectedBuffer.ToArray());
    }

    internal static async Task<SavedSyncSecrets?> LoadAsync(string root)
    {
        var path = Path.Combine(root, FileName);
        if (!File.Exists(path)) return null;
        try
        {
            var cipher = await File.ReadAllBytesAsync(path);
            var provider = new DataProtectionProvider();
            var plainBuffer = await provider.UnprotectAsync(cipher.AsBuffer());
            var plain = plainBuffer.ToArray();
            try
            {
                return JsonSerializer.Deserialize<SavedSyncSecrets>(plain);
            }
            finally
            {
                Array.Clear(plain, 0, plain.Length);
            }
        }
        catch
        {
            return null;
        }
    }

    internal static void Delete(string root)
    {
        var path = Path.Combine(root, FileName);
        if (File.Exists(path)) File.Delete(path);
    }

    /// Ghi file DPAPI cho bất kỳ secret cục bộ nào (khóa DB, passphrase khóa ghi chú…).
    internal static async Task ProtectFileAsync(string path, byte[] plain)
    {
        var provider = new DataProtectionProvider("LOCAL=user");
        var protectedBuffer = await provider.ProtectAsync(plain.AsBuffer());
        Array.Clear(plain, 0, plain.Length);
        await File.WriteAllBytesAsync(path, protectedBuffer.ToArray());
    }

    /// Đọc file DPAPI; trả về null khi file lỗi hoặc bị hỏng thay vì chết app.
    internal static async Task<byte[]?> UnprotectFileAsync(string path)
    {
        if (!File.Exists(path)) return null;
        try
        {
            var cipher = await File.ReadAllBytesAsync(path);
            var provider = new DataProtectionProvider();
            var plainBuffer = await provider.UnprotectAsync(cipher.AsBuffer());
            var plain = plainBuffer.ToArray();
            try { return plain; }
            finally { Array.Clear(plain, 0, plain.Length); }
        }
        catch { return null; }
    }
}
