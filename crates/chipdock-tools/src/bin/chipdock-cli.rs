use chipdock_control::{ApiError, ErrorCode, MAX_PAGE, Request, call};
use chipdock_tools::{ConnectionOptions, FormatOption};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "chipdock-cli",
    version,
    about = "Control ChipDock's open serial sessions over local IPC"
)]
struct Cli {
    /// Print a JSON object on stdout, including when a command fails.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Sessions,
    Ports,
    Status {
        session_id: u64,
    },
    Records {
        session_id: u64,
        #[arg(long, default_value_t = 0)]
        cursor: u64,
        #[arg(long, default_value_t = MAX_PAGE)]
        limit: u32,
    },
    Protocols {
        session_id: u64,
    },
    Connect {
        session_id: u64,
        path: String,
        #[command(flatten)]
        options: ConnectionOptions,
    },
    Disconnect {
        session_id: u64,
    },
    Send {
        session_id: u64,
        #[arg(long, value_enum, default_value = "text")]
        format: FormatOption,
        /// Text, HEX bytes, or arguments to the selected command template.
        #[arg(allow_hyphen_values = true)]
        input: String,
    },
    /// Select a named send template, or omit the name to clear it.
    SelectProtocol {
        session_id: u64,
        name: Option<String>,
    },
    OpenPlot {
        session_id: u64,
    },
}

impl TryFrom<Command> for Request {
    type Error = ApiError;
    fn try_from(command: Command) -> Result<Self, Self::Error> {
        Ok(match command {
            Command::Sessions => Self::Sessions,
            Command::Ports => Self::Ports,
            Command::Status { session_id } => Self::Status { session_id },
            Command::Records {
                session_id,
                cursor,
                limit,
            } => Self::Records {
                session_id,
                cursor,
                limit,
            },
            Command::Protocols { session_id } => Self::Protocols { session_id },
            Command::Connect {
                session_id,
                path,
                options,
            } => Self::Connect {
                session_id,
                path,
                reconnect: options.reconnect,
                config: options.try_into()?,
            },
            Command::Disconnect { session_id } => Self::Disconnect { session_id },
            Command::Send {
                session_id,
                format,
                input,
            } => Self::Send {
                session_id,
                format: format.into(),
                input,
            },
            Command::SelectProtocol { session_id, name } => {
                Self::SelectProtocol { session_id, name }
            }
            Command::OpenPlot { session_id } => Self::OpenPlot { session_id },
        })
    }
}

fn error_exit(error: ApiError, json_output: bool) -> ExitCode {
    if json_output {
        println!("{}", json!({"error": error}));
    } else {
        eprintln!("{error}");
    }
    ExitCode::from(match error.code {
        ErrorCode::OperationFailed => 1,
        ErrorCode::InvalidRequest => 2,
        ErrorCode::Unavailable => 3,
        ErrorCode::Disabled | ErrorCode::ReadOnly => 4,
        ErrorCode::SessionNotFound | ErrorCode::PortNotFound => 5,
        ErrorCode::PortBusy | ErrorCode::Busy | ErrorCode::NotConnected => 6,
        ErrorCode::VersionMismatch => 7,
        ErrorCode::Timeout | ErrorCode::OutcomeUnknown => 8,
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error)
            if error.use_stderr()
                && args
                    .iter()
                    .take_while(|arg| *arg != "--")
                    .any(|arg| arg == "--json") =>
        {
            return error_exit(
                ApiError::new(ErrorCode::InvalidRequest, error.to_string()),
                true,
            );
        }
        Err(error) => error.exit(),
    };
    let request = match cli.command.try_into() {
        Ok(request) => request,
        Err(error) => return error_exit(error, cli.json),
    };
    match call(request).await {
        Ok(result) => {
            if cli.json {
                println!("{}", json!({"result": result}));
            } else {
                println!("{result:#}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => error_exit(error, cli.json),
    }
}
