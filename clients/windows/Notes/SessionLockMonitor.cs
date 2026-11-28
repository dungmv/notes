using System.Runtime.InteropServices;

namespace Notes;

// Theo dõi sự kiện khóa phiên (Win+L) của Windows qua WTSRegisterSessionNotification
// trên một cửa sổ ẩn riêng, tránh phụ thuộc vào wndproc của MainWindow WinUI.
internal static class SessionLockMonitor
{
    private const int WTS_SESSION_LOCK = 0x7;
    private const int WM_WTSSESSION_CHANGE = 0x02B1;
    private const int NOTIFY_FOR_THIS_SESSION = 0;
    private const int WM_POWERBROADCAST = 0x0218;
    private const int PBT_APMSUSPEND = 0x0004;

    private static IntPtr window;
    private static WndProc? proc;
    private static Action? onLock;

    internal delegate IntPtr WndProc(IntPtr hWnd, int message, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    internal struct WndClass
    {
        public uint Style;
        public IntPtr WndProc;
        public int ClsExtra;
        public int WndExtra;
        public IntPtr Instance;
        public IntPtr Icon;
        public IntPtr Cursor;
        public IntPtr Background;
        [MarshalAs(UnmanagedType.LPWStr)] public string? MenuName;
        [MarshalAs(UnmanagedType.LPWStr)] public string ClassName;
        public IntPtr IconSm;
    }

    internal static void Start(Action onLock)
    {
        if (window != IntPtr.Zero)
        {
            SessionLockMonitor.onLock = onLock;
            return;
        }
        SessionLockMonitor.onLock = onLock;
        var started = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            try
            {
                proc = HandleMessage;
                var cls = new WndClass
                {
                    WndProc = Marshal.GetFunctionPointerForDelegate(proc),
                    Instance = GetModuleHandleW(null),
                    ClassName = "NotesSessionLockMonitor",
                };
                RegisterClassW(ref cls);
                window = CreateWindowExW(0, cls.ClassName, "", 0, 0, 0, 0, 0,
                    IntPtr.Zero, IntPtr.Zero, cls.Instance, IntPtr.Zero);
                WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION);
                started.TrySetResult();
                while (GetMessageW(out var message, IntPtr.Zero, 0, 0) > 0)
                {
                    TranslateMessage(ref message);
                    DispatchMessageW(ref message);
                }
            }
            catch { started.TrySetResult(); }
        })
        { IsBackground = true, Name = "notes-session-lock" };
        thread.Start();
        started.Task.Wait(TimeSpan.FromSeconds(3));
    }

    private static IntPtr HandleMessage(IntPtr handle, int message, IntPtr wParam, IntPtr lParam)
    {
        if (message == WM_WTSSESSION_CHANGE && wParam.ToInt64() == WTS_SESSION_LOCK ||
            message == WM_POWERBROADCAST && wParam.ToInt64() == PBT_APMSUSPEND)
            onLock?.Invoke();
        return DefWindowProcW(handle, message, wParam, lParam);
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Msg
    {
        public IntPtr Hwnd;
        public uint Message;
        public IntPtr WParam;
        public IntPtr LParam;
        public uint Time;
        public int PointX;
        public int PointY;
    }

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern ushort RegisterClassW(ref WndClass wndClass);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateWindowExW(int extended, string className, string name,
        int style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr extra);

    [DllImport("user32.dll")]
    private static extern int GetMessageW(out Msg message, IntPtr filter, int min, int max);

    [DllImport("user32.dll")]
    private static extern bool TranslateMessage(ref Msg message);

    [DllImport("user32.dll")]
    private static extern IntPtr DispatchMessageW(ref Msg message);

    [DllImport("user32.dll")]
    private static extern IntPtr DefWindowProcW(IntPtr handle, int message, IntPtr wParam, IntPtr lParam);

    [DllImport("wtsapi32.dll")]
    private static extern bool WTSRegisterSessionNotification(IntPtr handle, int flags);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandleW(string? module);
}

// Global hotkey Win+Alt+N mở Quick Note từ ứng dụng bất kỳ, đăng ký trên một
// cửa sổ ẩn riêng (WinUI không expose wndproc của MainWindow).
internal static class HotKeyMonitor
{
    private const int MOD_ALT = 0x1;
    private const int MOD_WIN = 0x8;
    private const int VK_N = 0x4E;
    private const int HOTKEY_ID = 1;
    private const int WM_HOTKEY = 0x0312;

    private static IntPtr window;
    private static SessionLockMonitor.WndProc? proc;
    private static Action? onHotKey;

    internal static void Start(Action callback)
    {
        onHotKey = callback;
        if (window != IntPtr.Zero) return;
        var started = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            try
            {
                proc = HandleMessage;
                var cls = new SessionLockMonitor.WndClass
                {
                    WndProc = System.Runtime.InteropServices.Marshal
                        .GetFunctionPointerForDelegate(proc),
                    Instance = GetModuleHandleW(null),
                    ClassName = "NotesQuickNoteHotKey",
                };
                RegisterClassW(ref cls);
                window = CreateWindowExW(0, cls.ClassName, "", 0, 0, 0, 0, 0,
                    IntPtr.Zero, IntPtr.Zero, cls.Instance, IntPtr.Zero);
                RegisterHotKey(window, HOTKEY_ID, MOD_WIN | MOD_ALT, VK_N);
                started.TrySetResult();
                while (GetMessageW(out var message, IntPtr.Zero, 0, 0) > 0)
                {
                    TranslateMessage(ref message);
                    DispatchMessageW(ref message);
                }
            }
            catch { started.TrySetResult(); }
        })
        { IsBackground = true, Name = "notes-hotkey" };
        thread.Start();
        started.Task.Wait(TimeSpan.FromSeconds(3));
    }

    private static IntPtr HandleMessage(IntPtr handle, int message, IntPtr wParam, IntPtr lParam)
    {
        if (message == WM_HOTKEY && wParam.ToInt64() == HOTKEY_ID)
            onHotKey?.Invoke();
        return DefWindowProcW(handle, message, wParam, lParam);
    }

    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true,
        CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    private static extern ushort RegisterClassW(ref SessionLockMonitor.WndClass wndClass);

    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true,
        CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    private static extern IntPtr CreateWindowExW(int extended, string className, string name,
        int style, int x, int y, int width, int height, IntPtr parent, IntPtr menu,
        IntPtr instance, IntPtr extra);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern int GetMessageW(out SessionLockMonitor.Msg message,
        IntPtr filter, int min, int max);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool TranslateMessage(ref SessionLockMonitor.Msg message);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern IntPtr DispatchMessageW(ref SessionLockMonitor.Msg message);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern IntPtr DefWindowProcW(IntPtr handle, int message, IntPtr wParam, IntPtr lParam);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool RegisterHotKey(IntPtr handle, int id, int modifiers, uint key);

    [System.Runtime.InteropServices.DllImport("kernel32.dll",
        CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    private static extern IntPtr GetModuleHandleW(string? module);
}
