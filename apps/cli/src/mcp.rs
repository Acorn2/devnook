use devnook_core::{ipc::CoreClient, model::Project};
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
        "annotations":{"readOnlyHint":read_only,"destructiveHint":!read_only,"idempotentHint":read_only,"openWorldHint":false}})
}
fn tools(writable: bool) -> Vec<Value> {
    let string = json!({"type":"string","minLength":1});
    let mut tools = vec![
        tool(
            "list_projects",
            "查询项目，可按名称搜索",
            json!({"search":{"type":"string"}}),
            &[],
            true,
        ),
        tool(
            "get_project",
            "查询项目详情",
            json!({"project_id":string}),
            &["project_id"],
            true,
        ),
        tool(
            "list_services",
            "查询同一个 Core 的真实服务状态",
            json!({"project_id":string,"state":string}),
            &[],
            true,
        ),
        tool(
            "get_service_status",
            "查询服务配置与当前运行",
            json!({"service_id":string}),
            &["service_id"],
            true,
        ),
        tool(
            "list_ports",
            "查询 macOS TCP 绑定",
            json!({"port":{"type":"integer","minimum":1,"maximum":65535},"process_name":string}),
            &[],
            true,
        ),
        tool(
            "inspect_process",
            "查询进程身份、目录和父链",
            json!({"pid":{"type":"integer","minimum":1,"maximum":2147483647}}),
            &["pid"],
            true,
        ),
        tool(
            "get_service_logs",
            "查询已脱敏的最近日志",
            json!({"service_id":string,"limit":{"type":"integer","minimum":1,"maximum":1000},"query":{"type":"string","maxLength":1024}}),
            &["service_id"],
            true,
        ),
        tool("list_workspaces", "查询工作区配置", json!({}), &[], true),
        tool(
            "get_workspace_status",
            "查询工作区最近一次编排及历史",
            json!({"workspace_id":string}),
            &["workspace_id"],
            true,
        ),
    ];
    if writable {
        for (name, target) in [
            ("start_service", "service_id"),
            ("restart_service", "service_id"),
            ("start_workspace", "workspace_id"),
            ("stop_workspace", "workspace_id"),
        ] {
            tools.push(tool(name,"首次返回完整预览与 60 秒 confirmation_token。向用户展示预览并取得确认后，用同一对象和令牌再次调用。",json!({target:string,"confirmation_token":string}),&[target],false));
        }
        tools.push(tool("preview_project_scan","在授权根目录内只读发现项目，返回 scan_id；depth=0 按特征 / 模块声明发现，1–12 为深度补扫。用 get_scan_status 等待分组候选预览，不执行项目命令。",json!({"root_path":string,"depth":{"type":"integer","minimum":0,"maximum":12}}),&["root_path"],false));
        tools.push(tool(
            "get_scan_status",
            "查询本授权创建的扫描会话",
            json!({"scan_id":string}),
            &["scan_id"],
            true,
        ));
        tools.push(tool("import_projects","先返回候选预览；用户确认后以相同候选 ID 和 confirmation_token 再次调用。",json!({"scan_id":string,"candidate_ids":{"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string","minLength":1}},"confirmation_token":string}),&["scan_id","candidate_ids"],false));
        tools.push(tool(
            "stop_service",
            "向已授权的 DevNook 托管实例发送 SIGTERM，不操作外部进程",
            json!({"service_id":string}),
            &["service_id"],
            false,
        ));
        tools.push(tool("open_project","在已授权项目的 IDE/Finder/Terminal 打开项目",json!({"project_id":string,"target":{"type":"string","enum":["DEFAULT","IDEA","VSCODE","CURSOR","FINDER","TERMINAL"]}}),&["project_id"],false));
    }
    tools
}
fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("缺少参数 {key}"))
}
fn call(client: &CoreClient, name: &str, args: Value) -> Result<Value, String> {
    match name {
        "list_projects" | "get_project" => {
            let mut projects: Vec<Project> = client.projects()?;
            if name == "get_project" {
                return serde_json::to_value(
                    projects
                        .into_iter()
                        .find(|p| p.candidate.id == required(&args, "project_id").unwrap_or(""))
                        .ok_or("项目不存在")?,
                )
                .map_err(|e| e.to_string());
            }
            if let Some(search) = args["search"].as_str() {
                projects.retain(|p| {
                    p.candidate
                        .name
                        .to_lowercase()
                        .contains(&search.to_lowercase())
                });
            }
            Ok(json!({"projects":projects}))
        }
        "list_services" | "get_service_status" => {
            let mut services = client.services()?;
            if name == "get_service_status" {
                return serde_json::to_value(
                    services
                        .into_iter()
                        .find(|s| s.definition.id == required(&args, "service_id").unwrap_or(""))
                        .ok_or("服务不存在")?,
                )
                .map_err(|e| e.to_string());
            }
            if let Some(project) = args["project_id"].as_str() {
                services.retain(|s| s.definition.project_id == project);
            }
            if let Some(state) = args["state"].as_str() {
                services.retain(|s| {
                    s.current_run.as_ref().is_some_and(|r| {
                        serde_json::to_value(&r.state).ok().as_ref() == Some(&json!(state))
                    })
                });
            }
            Ok(json!({"services":services}))
        }
        "list_ports" => {
            let mut ports = client.ports()?;
            if let Some(port) = args["port"].as_u64() {
                ports.retain(|p| u64::from(p.port) == port);
            }
            if let Some(name) = args["process_name"].as_str() {
                ports.retain(|p| p.name.to_lowercase().contains(&name.to_lowercase()));
            }
            Ok(json!({"ports":ports}))
        }
        "inspect_process" => client.invoke("inspect_process", args),
        "get_service_logs" => {
            let service = required(&args, "service_id")?;
            let run = client
                .service_history(service)?
                .into_iter()
                .next()
                .ok_or("该服务还没有运行日志")?;
            serde_json::to_value(client.service_logs(
                &run.id,
                args["limit"].as_u64().unwrap_or(100).min(1000) as usize,
                args["query"].as_str().unwrap_or(""),
            )?)
            .map_err(|e| e.to_string())
        }
        "list_workspaces" => Ok(json!({"workspaces":client.workspaces()?})),
        "get_workspace_status" => {
            Ok(json!({"runs":client.workspace_history(required(&args,"workspace_id")?)?}))
        }
        "start_service" | "restart_service" | "start_workspace" | "stop_workspace" => {
            let workspace = name.ends_with("workspace");
            let id = required(
                &args,
                if workspace {
                    "workspace_id"
                } else {
                    "service_id"
                },
            )?;
            if let Some(token) = args["confirmation_token"].as_str() {
                // Core binds token to grant, target and action. The adapter checks the echoed
                // target as well, so a mistaken second call cannot silently act elsewhere.
                client.invoke(
                    if workspace {
                        "confirm_workspace_action"
                    } else {
                        "confirm_service_action"
                    },
                    json!({"token":token,"id":id,"action":name.split('_').next().unwrap()}),
                )
            } else {
                let preview: Value = client.invoke(
                    if workspace {
                        "prepare_workspace_action"
                    } else {
                        "prepare_service_action"
                    },
                    json!({"id":id,"action":name.split('_').next().unwrap()}),
                )?;
                Ok(
                    json!({"needs_confirmation":true,"confirmation_token":preview["token"],"preview":preview}),
                )
            }
        }
        "preview_project_scan" => {
            let depth = match args.get("depth") {
                None => 0,
                Some(value) => value
                    .as_u64()
                    .filter(|value| *value <= 12)
                    .ok_or("depth 应为 0（自动）或 1–12")? as usize,
            };
            let id = client.scan_start(required(&args, "root_path")?.into(), depth)?;
            Ok(json!({"scan_id":id,"status":"scanning"}))
        }
        "get_scan_status" => client.invoke("scan_status", json!({"id":required(&args,"scan_id")?})),
        "import_projects" => {
            let params = json!({"scan_id":required(&args,"scan_id")?,"ids":args["candidate_ids"],"token":args["confirmation_token"]});
            if args["confirmation_token"].is_string() {
                client.invoke("import_projects", params)
            } else {
                let preview: Value = client.invoke("prepare_import_projects", params)?;
                Ok(
                    json!({"needs_confirmation":true,"confirmation_token":preview["token"],"preview":preview}),
                )
            }
        }
        "stop_service" => {
            serde_json::to_value(client.stop_service(required(&args, "service_id")?)?)
                .map_err(|e| e.to_string())
        }
        "open_project" => {
            client.open_project(
                required(&args, "project_id")?,
                args["target"].as_str().unwrap_or("DEFAULT"),
            )?;
            Ok(json!({"opened":true}))
        }
        _ => Err("未知工具".into()),
    }
}
fn validate(args: &Value, schema: &Value) -> bool {
    let Some(object) = args.as_object() else {
        return false;
    };
    if schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| !object.contains_key(k.as_str().unwrap()))
    {
        return false;
    }
    object.iter().all(|(key, value)| {
        let Some(property) = schema["properties"].get(key) else {
            return false;
        };
        if property["type"] == "string" {
            let Some(s) = value.as_str() else {
                return false;
            };
            if property["minLength"]
                .as_u64()
                .is_some_and(|n| (s.len() as u64) < n)
                || property["maxLength"]
                    .as_u64()
                    .is_some_and(|n| s.len() as u64 > n)
            {
                return false;
            }
        } else if property["type"] == "array" {
            let Some(items) = value.as_array() else {
                return false;
            };
            if items.is_empty()
                || items.len() > 1000
                || items
                    .iter()
                    .any(|v| !v.as_str().is_some_and(|s| !s.is_empty()))
            {
                return false;
            }
        } else if property["type"] == "integer" {
            let Some(n) = value.as_u64() else {
                return false;
            };
            if property["minimum"].as_u64().is_some_and(|v| n < v)
                || property["maximum"].as_u64().is_some_and(|v| n > v)
            {
                return false;
            }
        }
        !property["enum"]
            .as_array()
            .is_some_and(|allowed| !allowed.contains(value))
    })
}
pub fn serve(client: CoreClient, writable: bool) -> Result<(), String> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let available = tools(writable);
    let mut initialized = false;
    let mut ready = false;
    let mut protocol = String::new();
    loop {
        let mut line = Vec::new();
        let count = (&mut reader)
            .take(1_048_577)
            .read_until(b'\n', &mut line)
            .map_err(|_| "读取 MCP 输入失败")?;
        if count == 0 {
            return Ok(());
        }
        if line.len() > 1_048_576 {
            return Err("MCP 消息超过 1 MiB 限制".into());
        }
        let parsed = serde_json::from_slice::<Value>(&line);
        let mut id = Value::Null;
        let result: Result<Value, (i32, String)> = (|| {
            let request = parsed.map_err(|_| (-32700, "JSON 解析失败".into()))?;
            id = request["id"].clone();
            let method = request["method"]
                .as_str()
                .ok_or((-32600, "无效请求".into()))?;
            if request["jsonrpc"] != "2.0"
                || !request.is_object()
                || (!id.is_null() && !id.is_string() && !id.is_i64() && !id.is_u64())
                || (request.get("id").is_some() && id.is_null())
            {
                id = Value::Null;
                return Err((-32600, "无效 JSON-RPC 请求".into()));
            }
            if method == "notifications/initialized" {
                if initialized {
                    ready = true;
                }
                return Ok(Value::Null);
            }
            if !request.as_object().unwrap().contains_key("id") {
                return Ok(Value::Null);
            }
            if method == "ping" {
                return Ok(json!({}));
            }
            if method == "initialize" {
                if initialized {
                    return Err((-32600, "连接已初始化".into()));
                }
                if !request["params"]["capabilities"].is_object()
                    || !request["params"]["clientInfo"]["name"].is_string()
                    || !request["params"]["clientInfo"]["version"].is_string()
                {
                    return Err((-32602, "initialize 需要 capabilities 和 clientInfo".into()));
                }
                let version = request["params"]["protocolVersion"]
                    .as_str()
                    .ok_or((-32602, "缺少 protocolVersion".into()))?;
                let version = if ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"]
                    .contains(&version)
                {
                    version
                } else {
                    "2025-11-25"
                };
                initialized = true;
                protocol = version.into();
                return Ok(
                    json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"devnook","version":env!("CARGO_PKG_VERSION")},"instructions":"查询共享 DevNook Core。控制工具需范围授权；needs_confirmation 响应必须先向用户展示并取得确认。"}),
                );
            }
            if !ready {
                return Err((
                    -32002,
                    "请先完成 initialize 与 notifications/initialized".into(),
                ));
            }
            match method {
                "tools/list" => {
                    let mut listed = available.clone();
                    if protocol == "2024-11-05" {
                        for tool in &mut listed {
                            tool.as_object_mut().unwrap().remove("annotations");
                        }
                    }
                    Ok(json!({"tools":listed}))
                }
                "tools/call" => {
                    let name = request["params"]["name"]
                        .as_str()
                        .ok_or((-32602, "缺少工具名称".into()))?;
                    let tool = available
                        .iter()
                        .find(|t| t["name"] == name)
                        .ok_or((-32602, "工具不存在或当前为只读模式".into()))?;
                    let args = request["params"]
                        .get("arguments")
                        .cloned()
                        .unwrap_or(json!({}));
                    if !validate(&args, &tool["inputSchema"]) {
                        return Err((-32602, "工具参数不符合 inputSchema".into()));
                    }
                    Ok(match call(&client, name, args) {
                        Ok(data) => {
                            let mut result = json!({"content":[{"type":"text","text":serde_json::to_string(&data).unwrap()}],"isError":false});
                            if ["2025-06-18", "2025-11-25"].contains(&protocol.as_str()) {
                                result["structuredContent"] = if data.is_object() {
                                    data
                                } else {
                                    json!({"result":data})
                                };
                            }
                            result
                        }
                        Err(error) => {
                            json!({"content":[{"type":"text","text":error}],"isError":true})
                        }
                    })
                }
                _ => Err((-32601, "方法不存在".into())),
            }
        })();
        // Notifications have no response; parse/invalid-request errors carry a null id.
        if id.is_null() && result.is_ok() {
            continue;
        }
        let response = match result {
            Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        serde_json::to_writer(&mut writer, &response).map_err(|_| "写入 MCP 响应失败")?;
        writer
            .write_all(b"\n")
            .and_then(|_| writer.flush())
            .map_err(|_| "MCP 客户端已断开")?;
    }
}
