use std::io::Read;

use bytes::Bytes;
use reqwest::{blocking::{Client, Response}, header::CONTENT_TYPE, Url};

use crate::Result;
use crate::Error;

pub(crate) struct GetHandle(Client);

#[derive(Debug)]
pub enum Data {
    HTML(Url, String),
    OTHER(Url, Bytes)
}

impl Data {
    pub fn is_html(&self) -> bool {
        match &self {
            Data::HTML(_,_) => true,
            _ => false
        }
    }
    
    pub fn get_url(&self) -> &Url {
        match &self {
            Data::HTML(url, _) => url,
            Data::OTHER(url, _) => url
        }
    }
}

impl GetHandle {

    pub(crate) fn new() -> Self {
        GetHandle(Client::new())
    }

    pub(crate) fn get_url(&self, url: Url) -> Result<Data> {
        let mut resp = self.0.get(url.clone())
        .send().map_err(|e| (url.clone(), e))?;

        print!("Status for {}: {}", url, resp.status());
        if resp.status().is_server_error() || resp.status().is_client_error() {
            println!();
            return Err
            (
                Error::from((url, resp.error_for_status().unwrap_err()))
            )
        } else {
            println!(", Type: {:?}", resp.headers().get(CONTENT_TYPE).unwrap());
        }

        match check_html(&resp, &url) {
            true => {
                let mut body = String::new();
                resp.read_to_string(&mut body).map_err(|e| (url.clone(), e))?;
                Ok(Data::HTML(url, body))
            },
            false => {
                let content = resp.bytes().map_err(|e| (url.clone(), e))?;
                Ok(Data::OTHER(url, content))
            }
        }

        
    }

}


fn check_html(resp: &Response, url: &Url) -> bool {
    let ctype = resp.headers()
        .get(CONTENT_TYPE).map_or(None, |val| val.to_str().ok());
    
        if let Some(ctype) = ctype {
            if ctype.starts_with("text/html") {
                true
            } else if url.as_str().ends_with('/') && ctype.contains("charset=UTF-8") {
                true
            } else {
                false
            }
        } else {
            false
        }
    
}