use crate::GlobalOptions;
use crate::remote::client::{ClientMode, send_request};
use crate::remote::msg::watch::{WatchRequest, WatchResponse};
use rain_core::config::Config;

pub fn watch(
    config: &Config,
    target: &str,
    args: Vec<String>,
    options: &GlobalOptions,
    mode: ClientMode,
) -> Result<(), ()> {
    let custom_config = options.parse_config()?;
    let root = options.resolve_entrypoint()?;
    let watch_response = send_request(
        config,
        WatchRequest {
            root,
            target: target.to_owned(),
            args,
            resolve: options.resolve,
            offline: options.offline,
            seal: options.seal,
            host_override: options.host.clone(),
            custom_config,
            verification: options.verification,
            unused: options.unused,
            no_exec: options.no_exec,
        },
        |progress| {
            eprintln!("{progress:?}");
        },
        mode,
    )
    .map_err(|err| {
        eprintln!("{err}");
    })?;
    handle_watch_response(&watch_response);
    Ok(())
}

fn handle_watch_response(watch_response: &WatchResponse) {
    let WatchResponse { elapsed } = watch_response;
    eprintln!("Watch finished after {elapsed:.1?}");
}
