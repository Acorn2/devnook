mod mcp;
use clap::{Args, Parser, Subcommand};
use devnook_core::{
    access::{GrantInput, RequestContext},
    ipc::{self, CoreClient},
    service_model::*,
    supervisor::RunnerLauncher,
    workspace::*,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(
    name = "devnook",
    version,
    about = "DevNook 本地开发工作台 · 与桌面共享 Core"
)]
struct Cli {
    #[arg(long, global = true, env = "DEVNOOK_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "输出单行 JSON，预览与错误写入 stderr")]
    json: bool,
    #[arg(long, global = true, help = "确认本次操作；非交互环境的写操作必须提供")]
    yes: bool,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Ports {
        #[command(subcommand)]
        command: Option<PortsCommand>,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    Status,
}
#[derive(Subcommand)]
enum ProjectCommand {
    List,
    Scan {
        root: String,
        #[arg(
            long,
            default_value_t = 0,
            help = "0 按项目特征 / 模块声明发现；1–12 深度补扫非标准目录"
        )]
        depth: usize,
    },
    Import {
        scan_id: String,
        #[arg(required = true)]
        ids: Vec<String>,
    },
    Open {
        id: String,
        #[arg(long, default_value = "DEFAULT")]
        target: String,
    },
}
#[derive(Subcommand)]
enum ServiceCommand {
    List,
    Status {
        id: String,
    },
    Save {
        file: PathBuf,
    },
    Start {
        id: String,
    },
    Stop {
        id: String,
    },
    Restart {
        id: String,
    },
    Logs {
        id: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long)]
        run: Option<String>,
    },
    History {
        id: String,
    },
}
#[derive(Args)]
struct WorkspaceAction {
    id: String,
    #[arg(long, help = "返回运行 ID；后台继续编排")]
    no_wait: bool,
}
#[derive(Subcommand)]
enum WorkspaceCommand {
    List,
    Save { file: PathBuf },
    Status { id: String },
    History { id: String },
    Start(WorkspaceAction),
    Stop(WorkspaceAction),
}
#[derive(Subcommand)]
enum PortsCommand {
    Inspect { port: u16 },
}
#[derive(Subcommand)]
enum AgentCommand {
    List,
    Grant {
        name: String,
        #[arg(long)]
        service: Vec<String>,
        #[arg(long)]
        workspace: Vec<String>,
        #[arg(long)]
        project: Vec<String>,
        #[arg(long)]
        scan_root: Vec<String>,
    },
    Revoke {
        id: String,
    },
    Audit,
}
#[derive(Subcommand)]
enum McpCommand {
    Serve {
        #[arg(
            long,
            help = "允许范围授权控制；凭证仅从 DEVNOOK_MCP_TOKEN 环境变量读取"
        )]
        allow_write: bool,
    },
}
fn read_json<T: DeserializeOwned>(file: &PathBuf) -> Result<T, String> {
    let metadata = std::fs::metadata(file).map_err(|e| e.to_string())?;
    if metadata.len() > 131_072 {
        return Err("配置文件超过 128 KiB".into());
    }
    serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?)
        .map_err(|_| "配置 JSON 格式无效".into())
}
fn output<T: Serialize>(value: &T, compact: bool) -> Result<(), String> {
    println!(
        "{}",
        if compact {
            serde_json::to_string(value)
        } else {
            serde_json::to_string_pretty(value)
        }
        .map_err(|e| e.to_string())?
    );
    Ok(())
}
fn confirm(yes: bool, preview: &impl Serialize) -> Result<(), String> {
    eprintln!(
        "{}",
        serde_json::to_string_pretty(preview).map_err(|e| e.to_string())?
    );
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err("写操作需要 --yes；上方预览尚未执行".into());
    }
    eprint!("确认执行？输入 yes：");
    io::stderr().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    if answer.trim() == "yes" {
        Ok(())
    } else {
        Err("已取消".into())
    }
}
fn directory(value: Option<PathBuf>) -> Result<PathBuf, String> {
    value
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("Library/Application Support/com.devnook.desktop"))
        })
        .ok_or("无法确定数据目录，请使用 --data-dir".into())
}
fn run(cli: Cli) -> Result<(), String> {
    let client = CoreClient::new(
        &directory(cli.data_dir)?,
        std::env::current_exe().map_err(|e| e.to_string())?,
    )?
    .with_context(RequestContext {
        origin: "CLI".into(),
        credential: String::new(),
        read_only: false,
    });
    let result: Value = match cli.command {
        Commands::Status => client.invoke("ping", json!({}))?,
        Commands::Project { command } => match command {
            ProjectCommand::List => serde_json::to_value(client.projects()?).unwrap(),
            ProjectCommand::Scan { root, depth } => {
                let id = client.scan_start(root, depth)?;
                loop {
                    let status = client.scan_status(&id)?;
                    if status.status != "scanning" {
                        break serde_json::to_value(status).unwrap();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            ProjectCommand::Import { scan_id, ids } => {
                confirm(
                    cli.yes,
                    &json!({"action":"import_projects","scan_id":scan_id,"ids":ids,"preview":client.scan_status(&scan_id)?}),
                )?;
                serde_json::to_value(client.import_projects(&scan_id, ids)?).unwrap()
            }
            ProjectCommand::Open { id, target } => {
                confirm(
                    cli.yes,
                    &json!({"action":"open_project","id":id,"target":target}),
                )?;
                client.open_project(&id, &target)?;
                Value::Null
            }
        },
        Commands::Service { command } => {
            let restarting = matches!(&command, ServiceCommand::Restart { .. });
            match command {
                ServiceCommand::List => serde_json::to_value(client.services()?).unwrap(),
                ServiceCommand::Status { id } => serde_json::to_value(
                    client
                        .services()?
                        .into_iter()
                        .find(|s| s.definition.id == id)
                        .ok_or("服务不存在")?,
                )
                .unwrap(),
                ServiceCommand::Save { file } => {
                    let input: ServiceSave = read_json(&file)?;
                    confirm(
                        cli.yes,
                        &json!({"action":"save_service","definition":input.definition,"secret_keys":input.secret_values.keys().collect::<Vec<_>>()}),
                    )?;
                    serde_json::to_value(client.save_service(input)?).unwrap()
                }
                ServiceCommand::Start { id } | ServiceCommand::Restart { id } => {
                    let action = if restarting { "restart" } else { "start" };
                    let preview = client.prepare_service_action(&id, action)?;
                    confirm(cli.yes, &preview)?;
                    let run = client.confirm_service_action(&preview.token)?;
                    if !run.state.active() {
                        return Err(run.error.unwrap_or("服务启动失败".into()));
                    }
                    serde_json::to_value(run).unwrap()
                }
                ServiceCommand::Stop { id } => {
                    confirm(
                        cli.yes,
                        &json!({"action":"stop_service","service":client.service_history(&id)?.first()}),
                    )?;
                    serde_json::to_value(client.stop_service(&id)?).unwrap()
                }
                ServiceCommand::History { id } => {
                    serde_json::to_value(client.service_history(&id)?).unwrap()
                }
                ServiceCommand::Logs {
                    id,
                    limit,
                    query,
                    run,
                } => {
                    let run = run
                        .or_else(|| {
                            client
                                .service_history(&id)
                                .ok()?
                                .first()
                                .map(|r| r.id.clone())
                        })
                        .ok_or("没有运行日志")?;
                    serde_json::to_value(client.service_logs(&run, limit, &query)?).unwrap()
                }
            }
        }
        Commands::Workspace { command } => {
            let stopping = matches!(&command, WorkspaceCommand::Stop(_));
            match command {
                WorkspaceCommand::List => serde_json::to_value(client.workspaces()?).unwrap(),
                WorkspaceCommand::Save { file } => {
                    let group: WorkspaceGroup = read_json(&file)?;
                    confirm(cli.yes, &group)?;
                    serde_json::to_value(client.save_workspace(group)?).unwrap()
                }
                WorkspaceCommand::Status { id } => {
                    serde_json::to_value(client.workspace_history(&id)?.first()).unwrap()
                }
                WorkspaceCommand::History { id } => {
                    serde_json::to_value(client.workspace_history(&id)?).unwrap()
                }
                WorkspaceCommand::Start(args) | WorkspaceCommand::Stop(args) => {
                    let action = if stopping { "stop" } else { "start" };
                    let preview = client.prepare_workspace_action(&args.id, action)?;
                    confirm(cli.yes, &preview)?;
                    let mut run = client.confirm_workspace_action(&preview.token)?;
                    eprintln!("workspace run: {}", run.id);
                    while !args.no_wait && run.busy() {
                        std::thread::sleep(Duration::from_millis(150));
                        run = client
                            .workspace_history(&args.id)?
                            .into_iter()
                            .find(|r| r.id == run.id)
                            .ok_or("工作区记录不存在")?;
                    }
                    output(&run, cli.json)?;
                    if ["FAILED", "ROLLBACK_FAILED"].contains(&run.state.as_str()) {
                        return Err(run.error.unwrap_or("工作区执行失败".into()));
                    }
                    return Ok(());
                }
            }
        }
        Commands::Ports { command } => {
            let ports = client.ports()?;
            match command {
                None => serde_json::to_value(ports).unwrap(),
                Some(PortsCommand::Inspect { port }) => {
                    let mut details = Vec::new();
                    for pid in ports
                        .iter()
                        .filter(|p| p.port == port)
                        .map(|p| p.pid)
                        .collect::<std::collections::BTreeSet<_>>()
                    {
                        details.push(client.inspect(pid)?);
                    }
                    serde_json::to_value(details).unwrap()
                }
            }
        }
        Commands::Agent { command } => match command {
            AgentCommand::List => client.invoke("agent_grants", json!({}))?,
            AgentCommand::Audit => client.invoke("access_audits", json!({}))?,
            AgentCommand::Grant {
                name,
                service,
                workspace,
                project,
                scan_root,
            } => {
                let input = GrantInput {
                    name,
                    service_ids: service,
                    workspace_ids: workspace,
                    project_ids: project,
                    scan_roots: scan_root,
                };
                confirm(
                    cli.yes,
                    &json!({"action":"create_agent_grant","scope":input,"note":"包含工作区和服务的所有依赖；配置变化后需重新授权"}),
                )?;
                client.invoke("create_agent_grant", json!({"input":input}))?
            }
            AgentCommand::Revoke { id } => {
                confirm(cli.yes, &json!({"action":"revoke_agent_grant","id":id}))?;
                client.invoke("revoke_agent_grant", json!({"id":id}))?
            }
        },
        Commands::Mcp {
            command: McpCommand::Serve { allow_write },
        } => {
            let credential = if allow_write {
                std::env::var("DEVNOOK_MCP_TOKEN")
                    .map_err(|_| "--allow-write 需要 DEVNOOK_MCP_TOKEN")?
            } else {
                String::new()
            };
            return mcp::serve(
                client.with_context(RequestContext {
                    origin: "MCP".into(),
                    credential,
                    read_only: !allow_write,
                }),
                allow_write,
            );
        }
    };
    output(&result, cli.json)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let internal = match args.get(1).map(String::as_str) {
        Some("--devnook-worker") => Some(devnook_core::service_worker::from_stdin()),
        Some("--devnook-core") => Some(
            args.get(2)
                .ok_or("缺少 Core 数据目录".into())
                .and_then(|p| ipc::serve(&PathBuf::from(p), RunnerLauncher::current()?)),
        ),
        _ => None,
    };
    let result = internal.unwrap_or_else(|| run(Cli::parse()));
    if let Err(error) = result {
        eprintln!("DevNook: {error}");
        std::process::exit(1);
    }
}
