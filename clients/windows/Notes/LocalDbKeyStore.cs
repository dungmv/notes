using System.Security.Cryptography;
using System.Text;

namespace Notes;

// Khóa cục bộ của kho SQLite (SQLCipher). Sinh ngẫu nhiên 32 byte lần đầu chạy
// và lưu qua DPAPI trong thư mục dữ liệu, không bao giờ ghi dạng rõ lên đĩa.
internal static class LocalDbKeyStore
{
    private const string FileName = "local-db-key.dpapi";

    internal static async Task<string> LoadOrCreateAsync(string root)
    {
        Directory.CreateDirectory(root);
        var path = Path.Combine(root, FileName);
        var existing = await SecureKeyStore.UnprotectFileAsync(path);
        if (existing is { Length: 32 }) return ToHex(existing);

        var key = RandomNumberGenerator.GetBytes(32);
        await SecureKeyStore.ProtectFileAsync(path, key);
        return ToHex(key);
    }

    /// Cho ngữ cảnh đồng bộ (harness cũ gọi từ method void). Phải chạy qua
    /// Task.Run: gọi GetResult() trực tiếp trên UI thread sẽ deadlock với
    /// completion của DPAPI WinRT.
    internal static string LoadOrCreate(string root) =>
        Task.Run(() => LoadOrCreateAsync(root)).GetAwaiter().GetResult();

    private static string ToHex(byte[] bytes) => Convert.ToHexString(bytes).ToLowerInvariant();
}

// Mật khẩu khóa ghi chú được nhớ cho mở nhanh bằng Windows Hello. Việc kiểm
// chứng sinh trắc học do UserConsentVerifier thực hiện trước khi đọc secret.
internal static class HelloUnlockStore
{
    private const string FileName = "lock-hello.dpapi";

    internal static async Task SaveAsync(string root, string passphrase)
    {
        await SecureKeyStore.ProtectFileAsync(Path.Combine(root, FileName), Encoding.UTF8.GetBytes(passphrase));
    }

    internal static async Task DeleteAsync(string root)
    {
        var path = Path.Combine(root, FileName);
        if (File.Exists(path)) File.Delete(path);
        await Task.CompletedTask;
    }

    internal static async Task<string?> LoadAsync(string root)
    {
        var plain = await SecureKeyStore.UnprotectFileAsync(Path.Combine(root, FileName));
        return plain is null ? null : Encoding.UTF8.GetString(plain);
    }
}

// Windows Hello qua UserConsentVerifier: xác nhận sự hiện diện của người dùng
// trước khi dùng mật khẩu đã nhớ. Máy không có Hello vẫn mở bằng mật khẩu.
internal static class HelloUnlock
{
    internal static async Task<bool> IsAvailableAsync()
    {
        try
        {
            var status = await Windows.Security.Credentials.UI.UserConsentVerifier.CheckAvailabilityAsync();
            return status == Windows.Security.Credentials.UI.UserConsentVerifierAvailability.Available;
        }
        catch { return false; }
    }

    internal static async Task<bool> ConsentAsync(string message)
    {
        try
        {
            var result = await Windows.Security.Credentials.UI.UserConsentVerifier.RequestVerificationAsync(message);
            return result == Windows.Security.Credentials.UI.UserConsentVerificationResult.Verified;
        }
        catch { return false; }
    }
}
