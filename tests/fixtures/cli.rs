use std::{io::Write, time::Duration};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        !args
            .iter()
            .any(|a| a == "--accept-license" || a == "--accept-gdpr"),
        "tests must not accept terms"
    );
    match args.first().map(String::as_str) {
        Some("result") => {
            let result = r#"{"type":"result","isp":"Fixture","server":{"name":"Fixture"},"ping":{"latency":5},"download":{"bandwidth":1000000},"upload":{"bandwidth":500000},"result":{"url":"https://www.speedtest.net/result/1"}}"#;
            println!("{result}");
        }
        Some("flood") => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 100_000]);
        }
        Some("stderr-flood") => {
            let _ = std::io::stderr().write_all(&vec![b'x'; 100_000]);
        }
        Some("family") => {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["heartbeat", &args[1]])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            child.wait().unwrap();
        }
        Some("heartbeat") => loop {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&args[1])
                .unwrap();
            f.write_all(b".").unwrap();
            std::thread::sleep(Duration::from_millis(20));
        },
        Some("hold") => std::thread::sleep(Duration::from_secs(30)),
        _ => panic!("unknown fixture mode"),
    }
}
