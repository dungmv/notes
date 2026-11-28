use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

struct Client(*mut OfflineSession);

impl Client {
    fn open(path: &Path) -> Self {
        Self::open_with_key(path, None)
    }

    fn open_with_key(path: &Path, db_key: Option<&str>) -> Self {
        let mut handle = ptr::null_mut();
        let path = CString::new(path.to_str().unwrap()).unwrap();
        let key = db_key.map(|key| CString::new(key).unwrap());
        let key_ptr = key.as_ref().map_or(ptr::null(), |key| key.as_ptr());
        Self::read(unsafe { notes_offline_open(path.as_ptr(), key_ptr, &mut handle) });
        assert!(!handle.is_null());
        Self(handle)
    }

    /// Gọi và kỳ vọng lỗi với mã cụ thể, trả về toàn bộ phản hồi.
    fn call_error(&mut self, request: Value, code: &str) -> Value {
        let request = CString::new(request.to_string()).unwrap();
        let buffer = unsafe { notes_offline_call(self.0, request.as_ptr()) };
        let value: Value =
            serde_json::from_slice(unsafe { CStr::from_ptr(buffer) }.to_bytes()).unwrap();
        unsafe { crate::c_editor::notes_m0_string_free(buffer) };
        assert_eq!(value["code"], code, "{value}");
        value
    }

    fn call(&mut self, request: Value) -> Value {
        let request = CString::new(request.to_string()).unwrap();
        Self::read(unsafe { notes_offline_call(self.0, request.as_ptr()) })
    }

    fn read(buffer: *mut c_char) -> Value {
        let value: Value =
            serde_json::from_slice(unsafe { CStr::from_ptr(buffer) }.to_bytes()).unwrap();
        unsafe { crate::c_editor::notes_m0_string_free(buffer) };
        assert!(value.get("error").is_none(), "{value}");
        value["value"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe { notes_offline_free(self.0) };
    }
}

const PNG: &[u8] = include_bytes!("../../../../fixtures/m1/pixel.png");
const CHILD_ROOT: &str = "NOTES_TEST_AFTER_ACK_DIR";

fn test_db_key() -> String {
    "a1".repeat(32)
}

#[test]
fn locked_notes_flow_through_ffi() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let mut client = Client::open_with_key(root, Some(&test_db_key()));
    let id = client
        .call(json!({"type":"create_note","folder":"inbox"}))
        .as_str()
        .unwrap()
        .to_owned();
    let snapshot = client.call(json!({"type":"open_note","id":&id}));
    let block = snapshot["blocks"][0]["id"].as_str().unwrap().to_owned();
    client.call(json!({"type":"apply","batch":{"revision":0,"operations":[
        {"type":"replace","id":block,"start":0,"end":0,"text":"Ghi chú bí mật"}]}}));
    client.call(json!({"type":"paste_image","revision":1,"id":block,"start":7,"end":7,
        "png":base64::engine::general_purpose::STANDARD.encode(PNG)}));

    let status = client.call(json!({"type":"lock_status"}));
    assert_eq!(status["supported"], true);

    client.call(json!({"type":"lock_note","id":&id,"passphrase":"khoa-123"}));

    // Sau khóa: mở ghi chú bị chặn, danh sách giữ tiêu đề và cờ khóa,
    // tìm theo nội dung trả về rỗng nhưng tìm theo tiêu đề thì có.
    client.call_error(json!({"type":"open_note","id":&id}), "note_locked");
    let note = client.call(json!({"type":"note","id":&id}));
    assert_eq!(note["locked"], true);
    assert_eq!(note["snippet"], "");
    let list = client.call(json!({"type":"notes","folder":"all"}));
    let entry = list
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["id"] == id.as_str())
        .expect("locked note stays listed");
    assert_eq!(entry["locked"], true);
    assert!(
        client
            .call(json!({"type":"find","query":"bi mat"}))
            .as_array()
            .unwrap()
            .is_empty()
    );
    let found = client.call(json!({"type":"find","query":"ghi chu"}));
    assert_eq!(found[0]["id"], id.as_str());

    client.call_error(
        json!({"type":"unlock_note","id":&id,"passphrase":"sai"}),
        "wrong_passphrase",
    );

    let snapshot = client.call(json!({"type":"unlock_note","id":&id,"passphrase":"khoa-123"}));
    assert_eq!(snapshot["blocks"][0]["content"]["text"], "Ghi chú");
    let attachment = snapshot["blocks"][1]["attachment_id"].as_str().unwrap().to_owned();
    let image = client.call(json!({"type":"image_data","id":attachment}));
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(image["data"].as_str().unwrap())
            .unwrap(),
        PNG
    );

    // Sửa trong phiên mở, khóa lại, rồi khởi động lại bằng kho mới.
    client.call(json!({"type":"apply","batch":{"revision":2,"operations":[
        {"type":"replace","id":block,"start":7,"end":7,"text":"!"}]}}));
    client.call(json!({"type":"relock_note","id":&id}));
    drop(client);

    let mut client = Client::open_with_key(root, Some(&test_db_key()));
    let status = client.call(json!({"type":"lock_status"}));
    assert_eq!(status["locked_note_ids"][0], id.as_str());
    let snapshot = client.call(json!({"type":"unlock_note","id":&id,"passphrase":"khoa-123"}));
    assert_eq!(snapshot["blocks"][0]["content"]["text"], "Ghi chú!");
    drop(client);

    // Gỡ bảo vệ khôi phục tìm kiếm, metadata và blob dạng rõ.
    let mut client = Client::open_with_key(root, Some(&test_db_key()));
    client.call(json!({"type":"lock_remove_protection","id":&id,"passphrase":"khoa-123"}));
    let note = client.call(json!({"type":"note","id":&id}));
    assert_eq!(note["locked"], false);
    let found = client.call(json!({"type":"find","query":"bi mat"}));
    assert_eq!(found[0]["id"], id.as_str());
    client.call(json!({"type":"open_note","id":&id}));
    let path = client.call(json!({"type":"image_path","id":attachment}));
    assert_eq!(std::fs::read(path.as_str().unwrap()).unwrap(), PNG);
}

#[test]
fn lock_requires_encrypted_store_through_ffi() {
    let temp = tempfile::tempdir().unwrap();
    let mut client = Client::open(temp.path());
    let id = client.call(json!({"type":"create_note","folder":"inbox"}));
    client.call_error(
        json!({"type":"lock_note","id":id,"passphrase":"x"}),
        "lock_unsupported",
    );
    let status = client.call(json!({"type":"lock_status"}));
    assert_eq!(status["supported"], false);
}

#[test]
fn wrong_db_key_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    {
        Client::open_with_key(temp.path(), Some(&test_db_key()));
    }
    let path = CString::new(temp.path().to_str().unwrap()).unwrap();
    let key = CString::new("b2".repeat(32)).unwrap();
    let mut handle = ptr::null_mut();
    let reply = unsafe { notes_offline_open(path.as_ptr(), key.as_ptr(), &mut handle) };
    let value: Value =
        serde_json::from_slice(unsafe { CStr::from_ptr(reply) }.to_bytes()).unwrap();
    unsafe { crate::c_editor::notes_m0_string_free(reply) };
    assert_eq!(value["code"], "invalid_db_key", "{value}");
    assert!(handle.is_null());
}

#[test]
#[ignore = "invoked by reopen_after_terminating_writer"]
fn ack_writer() {
    let root = std::env::var_os(CHILD_ROOT).expect("test data directory");
    let mut client = Client::open(Path::new(&root));
    let id = client.call(json!({"type":"create_note","folder":"inbox"}));
    let snapshot = client.call(json!({"type":"open_note","id":id}));
    let block = &snapshot["blocks"][0]["id"];
    let update = client.call(json!({"type":"apply","batch":{
        "revision":0,"operations":[{"type":"replace","id":block,
            "start":0,"end":0,"text":"Tiếng Việt 😀"}]
    }}));
    assert_eq!(update["revision"], 1);
    let update = client.call(json!({
        "type":"paste_image","revision":1,"id":block,"start":5,"end":5,
        "png":base64::engine::general_purpose::STANDARD.encode(PNG)
    }));
    assert_eq!(update["revision"], 2);
    println!("ACK {}", id.as_str().unwrap());
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn reopen_after_terminating_writer() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "c_offline::tests::ack_writer",
            "--nocapture",
        ])
        .env(CHILD_ROOT, temp.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(id) = line.strip_prefix("ACK ") {
                let _ = send.send(id.to_owned());
                break;
            }
        }
    });

    let ack = receive.recv_timeout(Duration::from_secs(15));
    let killed = child.kill();
    child.wait().unwrap();
    reader.join().unwrap();
    let id = ack.expect("child ACK before termination");
    killed.expect("forcibly stop child after ACK");

    let mut client = Client::open(temp.path());
    let snapshot = client.call(json!({"type":"open_note","id":id}));
    assert_eq!(snapshot["revision"], 2);
    let blocks = snapshot["blocks"].as_array().unwrap();
    assert_eq!(blocks[0]["content"]["text"], "Tiếng");
    assert_eq!(blocks[1]["kind"], "image");
    assert_eq!(blocks[2]["content"]["text"], " Việt 😀");
    let path = client.call(json!({"type":"image_path","id":blocks[1]["attachment_id"]}));
    assert_eq!(std::fs::read(path.as_str().unwrap()).unwrap(), PNG);
    assert_eq!(
        client.call(json!({"type":"find","query":"tieng viet"}))[0]["id"],
        id
    );
}
