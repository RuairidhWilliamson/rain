mod github;
mod runner;

use std::{
    borrow::Cow,
    io::{Read as _, Write as _},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use httparse::Request;
use ipnet::IpNet;
use jsonwebtoken::EncodingKey;
use log::{error, info, warn};
use runner::Runner;

#[derive(Debug, serde::Deserialize)]
struct Config {
    addr: SocketAddr,
    github_app_id: github::model::AppId,
    github_app_key: PathBuf,
    github_webhook_secret: String,
    target_url: url::Url,
    seal: bool,
}

#[expect(clippy::unwrap_used)]
fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    env_logger::init();
    if let Err(err) = dotenv_result {
        warn!(".env could not be loaded: {err:#}");
    }
    let config = envy::from_env::<Config>()?;

    let key_raw = std::fs::read(&config.github_app_key).unwrap();
    let key = EncodingKey::from_rsa_pem(&key_raw).unwrap();

    let github_client = github::AppClient::new(github::AppAuth {
        app_id: config.github_app_id,
        key,
    });

    // let ipnets = [IpNet::from(IpAddr::V4(Ipv4Addr::LOCALHOST))];
    // let mut allowed_ipnets = Some(&ipnets);
    let allowed_ipnets: Option<&[IpNet]> = None;
    let listener = std::net::TcpListener::bind(config.addr)?;
    let server = Server {
        runner: Runner::new(config.seal),
        config,
        github_client,
    };
    loop {
        let (stream, addr) = listener.accept()?;
        if let Some(allowed_ipnets) = allowed_ipnets {
            if !allowed_ipnets
                .iter()
                .any(|ipnet| ipnet.contains(&addr.ip()))
            {
                warn!("connection {addr:?} did not match allowed ipnets");
                continue;
            }
        }
        if let Err(err) = server.handle_connection(stream, addr) {
            error!("handle connection: {err:#}");
        }
    }
}

struct Server {
    config: Config,
    runner: Runner,
    github_client: github::AppClient,
}

impl Server {
    fn handle_connection(&self, mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
        const OK_REPSONSE: &str = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        info!("connection {addr:?}");
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = Request::new(&mut headers);
        let mut buffer = [0u8; 1024];
        let len = stream.read(&mut buffer)?;
        let buffer = &buffer[..len];
        let parsed = request.parse(buffer)?;
        if parsed.is_partial() {
            return Err(anyhow!("partial http request"));
        }
        let handle_res = self.handle_request(&request, &mut stream, &buffer[parsed.unwrap()..]);
        let write_res = stream.write_all(OK_REPSONSE.as_bytes());
        handle_res?;
        write_res?;
        Ok(())
    }

    fn handle_request(
        &self,
        request: &Request,
        stream: &mut TcpStream,
        body_prefix: &[u8],
    ) -> Result<()> {
        match request.version {
            Some(0 | 1) => {}
            v => return Err(anyhow!("invalid http version: {v:?}")),
        }
        match request.path {
            Some("/webhook/github") => self.handle_github(request, stream, body_prefix),
            _ => Err(anyhow!("bad path")),
        }
    }

    fn handle_github(
        &self,
        request: &Request,
        stream: &mut TcpStream,
        body_prefix: &[u8],
    ) -> Result<()> {
        let content_type = find_header(request, "Content-Type").context("missing content type")?;
        if content_type != b"application/json" {
            return Err(anyhow!("unexpected content type"));
        }
        let user_agent = find_header(request, "User-Agent").context("missing user agent")?;
        if !user_agent.starts_with(b"GitHub-Hookshot/") {
            return Err(anyhow!("unexpected user agent"));
        }
        let raw_body = get_body(request, stream, body_prefix)?;
        self.verify_webhook_signature(request, &raw_body)?;

        let event_kind = std::str::from_utf8(
            find_header(request, "x-github-event").context("missing github event kind")?,
        )?;

        if event_kind != "check_suite" {
            return Err(anyhow!("unexpected event kind: {event_kind:?}"));
        }

        let check_suite_event: github::model::CheckSuiteEvent = serde_json::from_slice(&raw_body)?;

        if !matches!(
            check_suite_event.check_suite.status,
            Some(github::model::Status::Queued)
        ) {
            info!(
                "skipping check suite event {:?}",
                check_suite_event.check_suite.status
            );
            return Ok(());
        }

        let installation_client = self
            .github_client
            .auth_installation(check_suite_event.installation.id)?;
        info!("received {check_suite_event:?}");

        let owner = &check_suite_event.repository.owner.login;
        let repo = &check_suite_event.repository.name;
        let head_sha = &check_suite_event.check_suite.head_sha;

        let check_run = installation_client
            .create_check_run(
                owner,
                repo,
                github::model::CreateCheckRun {
                    name: String::from("rainci"),
                    head_sha: head_sha.into(),
                    status: github::model::Status::Queued,
                    details_url: Some(self.config.target_url.to_string()),
                    output: None,
                },
            )
            .context("create check run")?;
        info!("created check run {check_run:#?}");

        installation_client
            .update_check_run(
                owner,
                repo,
                check_run.id,
                github::model::PatchCheckRun {
                    status: Some(github::model::Status::InProgress),
                    ..Default::default()
                },
            )
            .context("update check run")?;

        let download = installation_client
            .download_repo_tar(owner, repo, head_sha)
            .context("download repo")?;
        let download_dir_name = format!("{owner}-{repo}-{head_sha}");
        let run_complete = self.runner.run(&download, &download_dir_name);

        let conclusion = if run_complete.success {
            github::model::CheckRunConclusion::Success
        } else {
            github::model::CheckRunConclusion::Failure
        };
        installation_client
            .update_check_run(
                owner,
                repo,
                check_run.id,
                github::model::PatchCheckRun {
                    status: Some(github::model::Status::Completed),
                    conclusion: Some(conclusion),
                    output: Some(github::model::CheckRunOutput {
                        title: String::from("rain run"),
                        summary: String::from("rain run complete"),
                        text: run_complete.output,
                    }),
                    ..Default::default()
                },
            )
            .context("update check run")?;

        self.runner.prune();

        Ok(())
    }

    fn verify_webhook_signature(&self, request: &Request, body: &[u8]) -> Result<()> {
        let raw_signature =
            find_header(request, "x-hub-signature-256").context("signature not present")?;
        let signature = std::str::from_utf8(raw_signature)?;
        let (algo, sig_hex) = signature
            .split_once('=')
            .context("header does not contain =")?;
        if algo != "sha256" {
            return Err(anyhow!("unknown algorithm"));
        }
        let sig = hex::decode(sig_hex).context("decode signature hex")?;
        let key = ring::hmac::Key::new(
            ring::hmac::HMAC_SHA256,
            self.config.github_webhook_secret.as_bytes(),
        );
        ring::hmac::verify(&key, body, &sig).context("verify signature")?;
        Ok(())
    }
}

fn find_header<'buf>(request: &Request<'_, 'buf>, header_name: &str) -> Option<&'buf [u8]> {
    request
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(header_name))
        .map(|h| h.value)
}

fn get_body<'buf>(
    request: &Request,
    stream: &'buf mut TcpStream,
    body_prefix: &'buf [u8],
) -> Result<Cow<'buf, [u8]>> {
    const MAXIMUM_BODY_LENGTH: usize = 102_400; // 100 KiB
    let content_length: usize = std::str::from_utf8(
        find_header(request, "Content-Length").context("missing content length")?,
    )?
    .parse()?;
    if content_length > MAXIMUM_BODY_LENGTH {
        return Err(anyhow!("content length exceeds max length"));
    }
    if content_length <= body_prefix.len() {
        return Ok(Cow::Borrowed(&body_prefix[..content_length]));
    }
    let mut body = vec![0u8; content_length];
    stream.read_exact(&mut body[body_prefix.len()..])?;
    body[..body_prefix.len()].copy_from_slice(body_prefix);

    Ok(Cow::Owned(body))
}
