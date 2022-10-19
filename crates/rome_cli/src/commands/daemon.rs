use rome_console::{markup, ConsoleExt};
use rome_lsp::ServerFactory;
use rome_service::{workspace::WorkspaceClient, RomeError, TransportError};
use std::{env, io::BufRead, io::Read};
use tokio::{
    io::{self, AsyncWriteExt, BufReader, BufWriter},
    runtime::Runtime,
    sync::mpsc,
};
use tracing::{debug_span, metadata::LevelFilter, Instrument, Metadata};
use tracing_subscriber::{
    layer::{Context, Filter},
    prelude::*,
    registry, Layer,
};
use tracing_tree::HierarchicalLayer;

use crate::{
    open_transport,
    service::{self, ensure_daemon, open_socket, read_message, run_daemon, write_message},
    CliSession, Termination,
};

pub(crate) fn start(mut session: CliSession) -> Result<(), Termination> {
    let rt = Runtime::new()?;
    let did_spawn = rt.block_on(ensure_daemon())?;

    if did_spawn {
        session.app.console.log(markup! {
            "The Rome server was successfully started"
        });
    } else {
        session.app.console.log(markup! {
            "The Rome server was already running"
        });
    }

    Ok(())
}

pub(crate) fn stop(mut session: CliSession) -> Result<(), Termination> {
    let rt = Runtime::new()?;

    if let Some(transport) = open_transport(rt)? {
        let client = WorkspaceClient::new(transport)?;
        match client.shutdown() {
            // The `ChannelClosed` error is expected since the server can
            // shutdown before sending a response
            Ok(()) | Err(RomeError::TransportError(TransportError::ChannelClosed)) => {}
            Err(err) => return Err(Termination::from(err)),
        };

        session.app.console.log(markup! {
            "The Rome server was successfully stopped"
        });
    } else {
        session.app.console.log(markup! {
            "The Rome server was not running"
        });
    }

    Ok(())
}

pub(crate) fn run_server() -> Result<(), Termination> {
    setup_tracing_subscriber();

    let rt = Runtime::new()?;
    let factory = ServerFactory::default();
    let cancellation = factory.cancellation();
    let span = debug_span!("Running Server", pid = std::process::id());

    rt.block_on(async move {
        tokio::select! {
            res = run_daemon(factory).instrument(span) => {
                match res {
                    Ok(never) => match never {},
                    Err(err) => Err(err.into()),
                }
            }
            _ = cancellation.notified() => {
                tracing::info!("Received shutdown signal");
                Ok(())
            }
        }
    })
}

pub(crate) fn print_socket() -> Result<(), Termination> {
    let rt = Runtime::new()?;
    rt.block_on(service::print_socket())?;
    Ok(())
}

pub(crate) fn lsp_proxy() -> Result<(), Termination> {
    let rt = Runtime::new()?;

    rt.block_on(async {
        ensure_daemon().await.expect("can't start rome");

        match open_socket().await.expect("connect error") {
            Some((owned_read_half, owned_write_half)) => {
                let (tx, mut rx) = mpsc::channel::<String>(8);

                // create spawn to write stdin content to socket
                rt.spawn(async move {
                    let mut socket_write = BufWriter::new(owned_write_half);
                    loop {
                        while let Some(msg) = rx.recv().await {
                            write_message(&mut socket_write, msg.as_bytes().to_vec())
                                .await
                                .expect("write error");
                        }
                    }
                });

                // create spawn to receive socket response to stdout
                rt.spawn(async move {
                    let mut stdout = io::stdout();
                    let mut socket_read = BufReader::new(owned_read_half);
                    loop {
                        let buf = read_message(&mut socket_read).await.expect("read err");
                        let res = String::from_utf8(buf).expect("response err");
                        print!("Content-Length: {}\r\n\r\n", res.len());
                        let _ = stdout.write_all(res.as_bytes()).await;
                        let _ = stdout.flush().await;
                    }
                });

                // forward stdin to socket
                let stdin = std::io::stdin();
                let mut stdin = stdin.lock();
                loop {
                    let mut buf = String::new();
                    stdin.read_line(&mut buf).unwrap();
                    if let Some(length) = buf.strip_prefix("Content-Length: ") {
                        let mut tmp_buf = String::new();
                        stdin.read_line(&mut tmp_buf).unwrap();

                        let length = length.trim().parse::<usize>().expect("parse error");
                        let mut msg: Vec<u8> = vec![0u8; length];

                        stdin.read_exact(&mut msg).unwrap();

                        let msg: String = String::from_utf8(msg).unwrap();
                        tx.send(msg).await.expect("send error");
                    } else {
                        break;
                    }
                }
            }
            None => print!("rome not start"),
        };
    });

    Ok(())
}

/// Setup the [tracing]-based logging system for the server
/// The events received by the subscriber are filtered at the `info` level,
/// then printed using the [HierarchicalLayer] layer, and the resulting text
/// is written to log files rotated on a hourly basis (in
/// `rome-logs/server.log.yyyy-MM-dd-HH` files inside the system temporary
/// directory)
fn setup_tracing_subscriber() {
    let logs_dir = env::temp_dir().join("rome-logs");
    let file_appender = tracing_appender::rolling::hourly(logs_dir, "server.log");

    registry()
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_indent_amount(2)
                .with_bracketed_fields(true)
                .with_targets(true)
                .with_ansi(false)
                .with_writer(file_appender)
                .with_filter(LoggingFilter),
        )
        .init();
}

/// Tracing filter enabling:
/// - All spans and events at level info or higher
/// - All spans and events at level debug in crates whose name starts with `rome`
struct LoggingFilter;

impl<S> Filter<S> for LoggingFilter {
    fn enabled(&self, meta: &Metadata<'_>, _cx: &Context<'_, S>) -> bool {
        let filter = if meta.target().starts_with("rome") {
            LevelFilter::DEBUG
        } else {
            LevelFilter::INFO
        };

        meta.level() <= &filter
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::DEBUG)
    }
}
