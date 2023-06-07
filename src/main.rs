#[macro_use]
extern crate log;
use futures::prelude::*;
use pop_launcher::{
    async_stdin, async_stdout, json_input_stream, PluginResponse, PluginSearchResult, Request,
};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use std::process::Command;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create("tmux-sessions.log").unwrap(),
    )
    .ok();

    let app = App::default();
    let mut requests = json_input_stream(async_stdin());

    while let Some(result) = requests.next().await {
        match result {
            Ok(request) => match request {
                Request::Activate(id) => debug!("Activate: {}", id),
                Request::Complete(id) => debug!("Complete: {}", id),
                Request::Search(query) => app.search(query).await,
                Request::Exit => break,
                _ => (),
            },
            Err(err) => debug!("ERR Got request: {:?}", err),
        }
    }
}

#[derive(Debug, Default)]
struct App {}

impl App {
    pub async fn search(&self, query: String) {
        PluginResponse::Append(PluginSearchResult {
            id: 1,
            name: query,
            description: "test".to_string(),
            icon: None,
            ..Default::default()
        });

        match Command::new("tmux").args(["ls"]).output() {
            Ok(result) => {
                let stdout = String::from_utf8(result.stdout).expect("stdout to be utf8");
                debug!("{}", stdout);
            }
            Err(err) => {
                debug!("ERR: {}", err);
            }
        }

        send(&mut async_stdout(), PluginResponse::Finished).await;
    }
}

pub async fn send<W: AsyncWrite + Unpin>(tx: &mut W, response: PluginResponse) {
    if let Ok(mut bytes) = serde_json::to_string(&response) {
        bytes.push('\n');
        let _ = tx.write_all(bytes.as_bytes()).await;
    }
}
