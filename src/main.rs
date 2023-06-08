#[macro_use]
extern crate log;
use chrono::prelude::*;
use chrono::{DateTime, Local, NaiveDateTime};
use futures::prelude::*;
use pop_launcher::{
    async_stdin, async_stdout, json_input_stream, PluginResponse, PluginSearchResult, Request,
};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::convert::Infallible;
use std::fs::File;
use std::process::Command;
use std::str::FromStr;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create("tmux-sessions.log").unwrap(),
    )
    .ok();

    let mut plugin = Plugin::default();
    let mut requests = json_input_stream(async_stdin());

    while let Some(result) = requests.next().await {
        match result {
            Ok(request) => match request {
                Request::Activate(id) => debug!("Activate: {}", id),
                Request::Complete(id) => debug!("Complete: {}", id),
                Request::Search(query) => plugin.search(query).await,
                Request::Exit => break,
                _ => (),
            },
            Err(err) => debug!("ERR Got request: {:?}", err),
        }
    }
}

#[derive(Debug)]
struct Plugin {
    out: blocking::Unblock<std::io::Stdout>,
}

impl Default for Plugin {
    fn default() -> Self {
        Plugin {
            out: async_stdout(),
        }
    }
}

impl Plugin {
    pub async fn search(&mut self, _query: String) {
        let res = match Command::new("tmux")
            .args(["ls", "-F", "#S:#{session_windows}:#{session_created}"])
            .output()
        {
            Ok(result) => {
                let stdout = String::from_utf8(result.stdout).expect("stdout to be utf8");
                debug!("{}", stdout);
                stdout
            }
            Err(err) => {
                debug!("ERR: {}", err);
                String::from("ERR")
            }
        };

        let lines = res
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let session = line.parse::<TmuxSession>().unwrap();
                let time = session.created.format("%Y-%m-%d %H:%M:%S %Z");
                PluginSearchResult {
                    id: i as u32,
                    name: format!("Attach to session: {}", session.name),
                    description: format!("{} Windows Time: {}", session.windows, time),
                    icon: None,
                    ..Default::default()
                }
            })
            .map(PluginResponse::Append);

        for line in lines {
            send(&mut self.out, line).await;
        }

        send(&mut self.out, PluginResponse::Finished).await;
    }
}

pub async fn send<W: AsyncWrite + Unpin>(tx: &mut W, response: PluginResponse) {
    if let Ok(mut bytes) = serde_json::to_string(&response) {
        bytes.push('\n');
        let _ = tx.write_all(bytes.as_bytes()).await;
    }
}

struct TmuxSession {
    name: String,
    windows: usize,
    created: DateTime<Local>,
}

impl FromStr for TmuxSession {
    type Err = Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.split(':').collect::<TmuxSession>())
    }
}

impl<'a> FromIterator<&'a str> for TmuxSession {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let name = iter.next().expect("Missing name");
        let windows = iter
            .next()
            .unwrap()
            .parse::<usize>()
            .expect("Missing windows");
        let created = iter
            .next()
            .unwrap()
            .parse::<i64>()
            .expect("Missing created");
        let created = NaiveDateTime::from_timestamp_opt(created, 0).unwrap();
        let created = Local.from_local_datetime(&created).unwrap();
        TmuxSession {
            name: String::from(name),
            windows,
            created,
        }
    }
}
