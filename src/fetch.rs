use std::fmt;
use std::io::Read;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use hyper::{status::StatusCode, Client};
use url::{ParseResult, Url, UrlParser};

use crate::parse;

#[derive(Debug, Clone)]
pub enum UrlState {
    Accessible(Url),
    BadStatus(Url, StatusCode),
    ConnectionFailed(Url),
    TimedOut(Url),
    Malformed(String),
}

impl fmt::Display for UrlState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UrlState::Accessible(ref url) => format!("✓ {}", url).fmt(f),
            UrlState::BadStatus(ref url, ref status) => format!("x {} ({})", url, status).fmt(f),
            UrlState::ConnectionFailed(ref url) => format!("x {} (connection failed)", url).fmt(f),
            UrlState::TimedOut(ref url) => format!("x {} (timed out)", url).fmt(f),
            UrlState::Malformed(ref s) => format!("x {} (malformed)", s).fmt(f),
        }
    }
}

fn build_url(domain: &str, path: &str) -> ParseResult<Url> {
    let base_url_string = format!("http://{}", domain);
    let base_url = Url::parse(&base_url_string).unwrap();

    let mut raw_url_parser = UrlParser::new();
    let url_parser = raw_url_parser.base_url(&base_url);

    url_parser.parse(path)
}

pub fn url_status(domain: &str, path: &str, timeout: u64) -> UrlState {
    match build_url(domain, path) {
        Ok(url) => {
            let (tx, rx) = channel();
            let req_tx = tx.clone();
            let u = url.clone();

            thread::spawn(move || {
                let client = Client::new();
                let url_string = url.serialize();
                let resp = client.get(&url_string).send();
                let _ = req_tx.send(match resp {
                    Ok(r) => {
                        if let StatusCode::Ok = r.status {
                            UrlState::Accessible(url)
                        } else {
                            UrlState::BadStatus(url, r.status)
                        }
                    }
                    Err(_) => UrlState::ConnectionFailed(url),
                });
            });

            thread::spawn(move || {
                thread::sleep(Duration::from_secs(timeout));
                let _ = tx.send(UrlState::TimedOut(u));
            });

            rx.recv().unwrap()
        }
        Err(_) => UrlState::Malformed(path.to_owned()),
    }
}

fn fetch(url: &Url) -> String {
    let client = Client::new();
    let url_string = url.serialize();
    let mut res = client
        .get(&url_string)
        .send()
        .ok()
        .expect("Failed to fetch url");

    let mut body = String::new();
    match res.read_to_string(&mut body) {
        Ok(_) => body,
        Err(_) => String::new(),
    }
}

pub fn fetch_many(url: &Url) -> Vec<String> {
    let html_str = fetch(url);
    let dom = parse::parse_html(&html_str);

    parse::get_urls(dom.document)
}
