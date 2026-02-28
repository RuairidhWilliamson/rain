use std::io::{self, Write as _};

use rain_core::config::Config;

use crate::remote::{
    client::{ClientMode, make_request_or_start},
    msg::{
        cache_inspect::{CacheInspectRequest, CacheInspectResponse},
        clean::CleanRequest,
        info::InfoRequest,
        prune::{PruneRequest, Pruned},
        shutdown::ShutdownRequest,
    },
};

pub fn init_template() -> Result<(), ()> {
    let mut f = std::fs::File::create_new("main.rain")
        .map_err(|err| eprintln!("could not create main.rain: {err}"))?;
    write!(f, include_str!("template_main.rain"))
        .map_err(|err| eprintln!("could not write main.rain: {err}"))?;
    f.flush()
        .map_err(|err| eprintln!("could not flush main.rain: {err}"))?;
    Ok(())
}

pub fn info(config: &Config, client_mode: ClientMode) -> Result<(), ()> {
    let info = make_request_or_start(config, InfoRequest, |()| {}, client_mode).map_err(|err| {
        eprintln!("{err}");
    })?;
    println!("{info:#?}");
    Ok(())
}

pub fn shutdown(config: &Config, client_mode: ClientMode) -> Result<(), ()> {
    make_request_or_start(config, ShutdownRequest, |()| {}, client_mode).map_err(|err| {
        eprintln!("{err}");
    })?;
    eprintln!("Server shutdown");
    Ok(())
}

pub fn inspect_config(config: &Config) {
    eprintln!("{config:#?}");
}

pub fn inspect_cache(config: &Config, client_mode: ClientMode) -> Result<(), ()> {
    let CacheInspectResponse {
        cache_size,
        entries,
    } = make_request_or_start(config, CacheInspectRequest, |()| {}, client_mode).map_err(
        |err| {
            eprintln!("{err}");
        },
    )?;
    eprintln!("Cache size is {cache_size}");
    for e in entries {
        eprintln!("{e}");
    }
    Ok(())
}

pub fn resolve(config: &Config, path: Option<String>) {
    let lines: Box<dyn Iterator<Item = String>> = if let Some(p) = path {
        Box::new(std::iter::once(p))
    } else {
        Box::new(io::stdin().lines().map(|s| s.expect("read stdin")))
    };
    for line in lines {
        let path = config.base_generated_dir.join(line);
        println!("{}", path.display());
    }
}

pub fn clean(config: &Config, mode: ClientMode) -> Result<(), ()> {
    println!("Will delete:");
    for p in config.clean_directories() {
        println!("  {}", p.display());
    }
    if inquire::Confirm::new("Delete all these directories recursively?")
        .prompt_skippable()
        .map_err(|err| {
            eprintln!("{err}");
        })?
        == Some(true)
    {
        let resp = make_request_or_start(config, CleanRequest, |()| {}, mode).map_err(|err| {
            eprintln!("{err}");
        })?;
        if resp.0.is_empty() {
            println!("Nothing to clean");
        } else {
            println!("Cleaned");
            for (p, s) in resp.0 {
                println!(
                    "  {:8} {}",
                    humansize::format_size(s, humansize::BINARY),
                    p.display(),
                );
            }
        }
    } else {
        println!("Did nothing");
    }
    Ok(())
}

pub fn prune(config: &Config, mode: ClientMode) -> Result<(), ()> {
    let Pruned { size, errors } = make_request_or_start(config, PruneRequest, |()| {}, mode)
        .map_err(|err| {
            eprintln!("{err}");
        })?;
    println!(
        "Pruned {:8}",
        humansize::format_size(size, humansize::BINARY)
    );
    if errors > 0 {
        println!("{errors} Errors");
    }
    Ok(())
}
