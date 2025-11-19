
#[cfg(unix)]
pub(crate) async fn listen_signal() -> anyhow::Result<()> {
    tokio::spawn(async {
        use tokio::signal::unix::{SignalKind, signal};

        let mut s = signal(SignalKind::hangup())?;
        let hangup = s.recv();
        let mut s = signal(SignalKind::terminate())?;
        let terminate = s.recv();
        let mut s = signal(SignalKind::interrupt())?;
        let interrupt = s.recv();
        let mut s = signal(SignalKind::quit())?;
        let quit = s.recv();

        tokio::select! {
            _ = hangup => {
                // println!("signal hangup");
            }
            _ = terminate => {
                // println!("signal terminate");
            }
            _ = interrupt => {
                // println!("signal interrupt");
            }
            _ = quit => {
                // println!("signal quit");
            }
        }
        Ok(())
    })
    .await?
}

#[cfg(not(unix))]
pub(crate) async fn listen_signal() -> anyhow::Result<()> {
    let () = std::future::pending().await;
    unreachable!();
}