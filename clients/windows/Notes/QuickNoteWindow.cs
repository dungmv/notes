using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Notes;

// Quick Note: cửa sổ nổi mở bằng Win+Alt+N từ bất kỳ ứng dụng nào. Ghi chú
// được lưu vào thư mục Ghi chú (inbox) qua một phiên OfflineSession riêng,
// commit theo đường lưu thật của core trước khi cửa sổ đóng.
internal sealed class QuickNoteWindow : Window
{
    private readonly RichEditBox editor = new()
    {
        AcceptsReturn = true,
        MinHeight = 200,
        MinWidth = 420,
    };
    private readonly string dataRoot;
    private OfflineSession? session;
    private bool saving;

    internal QuickNoteWindow(string dataRoot)
    {
        this.dataRoot = dataRoot;
        Title = "Quick Note";
        var panel = new StackPanel { Padding = new Thickness(12), Spacing = 8 };
        var hint = new TextBlock
        {
            Text = "Ghi nhanh (Ctrl+Enter hoặc đóng cửa sổ để lưu)",
            Opacity = 0.75,
        };
        panel.Children.Add(hint);
        panel.Children.Add(editor);
        Content = panel;
        AppWindow.Resize(new Windows.Graphics.SizeInt32(460, 320));
        editor.KeyDown += (_, args) =>
        {
            if (args.Key == VirtualKey.Enter &&
                Microsoft.UI.Input.InputKeyboardSource
                    .GetKeyStateForCurrentThread(VirtualKey.Control)
                    .HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down))
            {
                args.Handled = true;
                Close();
            }
        };
        Closed += (_, _) => _ = SaveAndCloseAsync();
        _ = OpenSessionAsync();
    }

    private async Task OpenSessionAsync()
    {
        session = await Task.Run(async () =>
            OfflineSession.Open(dataRoot, await LocalDbKeyStore.LoadOrCreateAsync(dataRoot)));
        editor.Focus(FocusState.Programmatic);
    }

    private async Task SaveAndCloseAsync()
    {
        var session = this.session;
        this.session = null;
        if (session is null || saving) return;
        saving = true;
        try
        {
            editor.Document.GetText(TextGetOptions.None, out var text);
            var content = text.TrimEnd('\r', '\n');
            if (content.Length > 0)
            {
                await Task.Run(() =>
                {
                    var id = session.Call<string>(new { type = "create_note", folder = "inbox" });
                    var snapshot = session.Call<EditorSnapshot>(new { type = "open_note", id });
                    var block = snapshot.Blocks[0].Id;
                    session.Apply(snapshot.Revision,
                        [new { type = "replace", id = block, cell = (object?)null,
                            start = 0, end = 0, text = content }],
                        false);
                });
            }
        }
        finally
        {
            session.Dispose();
        }
    }
}
