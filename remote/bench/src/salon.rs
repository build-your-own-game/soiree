//! A real salon, in a process of its own: `salon-door`.

use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::eventually;

/// Built here and not with the tests: salon may not compile while it moves,
/// and only the tests that need it should say so.
fn door() -> &'static PathBuf {
    static DOOR: OnceLock<PathBuf> = OnceLock::new();
    DOOR.get_or_init(|| {
        let bench = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let status = Command::new(env!("CARGO"))
            .current_dir(&bench)
            .args(["build", "--package", "salon-door"])
            .status()
            .expect("cargo");
        assert!(status.success(), "salon-door does not build");
        bench.join("target/debug/salon-door")
    })
}

/// A game that shows `kind`: `pad`, `draw`, or its `page`.
pub struct Salon {
    child: Child,
    pub port: u16,
    heard: Arc<Mutex<Vec<String>>>,
}

impl Salon {
    pub fn start(kind: &str) -> Salon {
        // Free on TCP and on UDP, which salon opens with the same number.
        let port = loop {
            let tcp = TcpListener::bind(("0.0.0.0", 0)).expect("a port");
            let port = tcp.local_addr().expect("a port").port();
            if UdpSocket::bind(("0.0.0.0", port)).is_ok() {
                break port;
            }
        };
        let dir = std::env::temp_dir().join(format!("remote-bench-salon-{port}"));
        let mut child = Command::new(door())
            .arg(port.to_string())
            .arg(&dir)
            .arg(kind)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("salon-door");
        let output = child.stdout.take().expect("its output");
        let heard = Arc::new(Mutex::new(Vec::new()));
        let hearing = Arc::clone(&heard);
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                hearing.lock().expect("lines").push(line);
            }
        });
        assert!(
            eventually(Duration::from_secs(10), || TcpStream::connect((
                "127.0.0.1",
                port
            ))
            .is_ok()),
            "salon never listened"
        );
        Salon { child, port, heard }
    }

    pub fn page(&self) -> String {
        format!("http://127.0.0.1:{}/play", self.port)
    }

    /// How many times the game heard this.
    pub fn heard(&self, line: &str) -> usize {
        let heard = self.heard.lock().expect("lines");
        heard.iter().filter(|heard| *heard == line).count()
    }

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.child.id().to_string())
            .status()
            .expect("kill");
        assert!(status.success(), "kill -{name}");
    }

    /// The kernel keeps accepting and acknowledging: the link looks alive.
    pub fn freeze(&self) {
        self.signal("STOP");
    }

    pub fn thaw(&self) {
        self.signal("CONT");
    }
}

impl Drop for Salon {
    fn drop(&mut self) {
        let _: std::io::Result<()> = self.child.kill();
        let _: std::io::Result<std::process::ExitStatus> = self.child.wait();
    }
}
