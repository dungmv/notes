use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

struct Client(*mut OfflineSession);

impl Client {
    fn open(path: &Path) -> Self {
        let mut handle = ptr::null_mut();
        let path = CString::new(path.to_str().unwrap()).unwrap();
        Self::read(unsafe { notes_offline_open(path.as_ptr(), &mut handle) });
        assert!(!handle.is_null());
        Self(handle)
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
