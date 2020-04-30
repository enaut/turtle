use std::io;
use std::env;
use std::process::{Stdio, exit};

use tokio::io::AsyncWriteExt;
use tokio::process::{Command, ChildStdin};

/// The environment variable that is set to indicate that the current process is a server process
pub const RENDERER_PROCESS_ENV_VAR: &str = "RUN_TURTLE_CANVAS";

#[derive(Debug)]
pub struct RendererServerProcess {
    /// A handle to the stdin of the child process
    child_stdin: ChildStdin,
}

impl RendererServerProcess {
    /// Spawn a new process for the renderer
    pub async fn spawn() -> Self {
        let current_exe = env::current_exe()
            .expect("Could not read path of the currently running executable");

        // The new process is the same executable as this process but with a special environment
        // variable passed in
        let mut child = Command::new(current_exe)
            .env(RENDERER_PROCESS_ENV_VAR, "true")
            // Pipe input so we can communicate with the spawned process
            //
            // stdout/stderr will be inherited from the current process
            .stdin(Stdio::piped())
            .spawn()
            .expect("failed to start separate process for renderer");

        let child_stdin = child.stdin.take()
            .expect("renderer process was not spawned with a handle to stdin");

        // Ensure the child process is spawned in the runtime so it can
        // make progress on its own while we send it input.
        tokio::spawn(async {
            // We want to await in a separate task because otherwise the current task would not
            // make any progress until the child process is complete
            let status = child.await
                .expect("child process encountered an error");

            if status.success() {
                // The window/renderer process was closed normally
                exit(0);
            } else {
                // Something went wrong, likely the other thread panicked
                exit(1);
            }
        });

        Self {child_stdin}
    }

    /// Writes the given bytes followed by a newline b'\n' to the stdin of the process
    ///
    /// Unlike `std::io::Write::write`, this returns an error in the case all of the bytes could
    /// not be written for some reason.
    pub async fn writeln<S: AsRef<[u8]>>(&mut self, data: S) -> io::Result<()> {
        let data = data.as_ref();
        let bytes_written = self.child_stdin.write(data).await?;
        self.child_stdin.write_u8(b'\n').await?;

        if bytes_written == data.len() {
            Ok(())

        } else {
            // From the docs: "This typically means that an operation could only succeed if it
            // wrote a particular number of bytes but only a smaller number of bytes could be
            // written."
            let err_msg = format!("expected to write `{}` bytes, but actually wrote `{}`", data.len(), bytes_written);
            Err(io::Error::new(io::ErrorKind::WriteZero, err_msg))
        }
    }
}