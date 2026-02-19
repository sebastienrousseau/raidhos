use raidhos_core as core;
use serde::Serialize;

#[derive(Serialize)]
struct HelperResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "".to_string());

    match cmd.as_str() {
        "list-disks" => {
            let resp = match core::list_disks() {
                Ok(disks) => HelperResponse {
                    ok: true,
                    data: Some(disks),
                    error: None,
                },
                Err(err) => HelperResponse::<Vec<core::DiskInfo>> {
                    ok: false,
                    data: None,
                    error: Some(err.to_string()),
                },
            };

            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        }
        "install" => {
            let device = args.next().unwrap_or_default();
            let payload = args.next().unwrap_or_else(|| "1.1.10".to_string());
            let wipe = args.next().as_deref() == Some("true");
            let dry_run = args.next().as_deref() == Some("true");

            let sink = StdoutSink;
            let req = core::InstallRequest {
                device,
                payload_version: payload,
                wipe,
                dry_run,
            };

            let resp = match core::install(req, &sink) {
                Ok(_) => HelperResponse::<()> {
                    ok: true,
                    data: Some(()),
                    error: None,
                },
                Err(err) => HelperResponse::<()> {
                    ok: false,
                    data: None,
                    error: Some(err.to_string()),
                },
            };

            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        }
        _ => {
            eprintln!("usage: raidhos-priv-helper <list-disks|install> [args]");
            std::process::exit(2);
        }
    }
}

struct StdoutSink;

impl core::ProgressSink for StdoutSink {
    fn emit(&self, event: core::ProgressEvent) {
        let line = format!("{}: {}", event.phase, event.message);
        eprintln!("{line}");
    }
}
