
use std::collections::HashSet;

use reqwest::Url;
use select::document::Document;
use select::predicate::{Attr, Predicate};

use crate::get::Data;
use crate::Result;


const SF_OPTIONS: sanitize_filename::Options = sanitize_filename::Options {
    truncate: false,
    replacement: "_",
    windows: true
};

pub(crate) fn filter_visited(found_urls: HashSet<Url>, visited: &HashSet<Url>, origin_url: &Url, host_only: bool) -> HashSet<Url> {
    found_urls
        .difference(visited)
        .filter(|url| {
            if host_only {
                origin_url.host_str() == url.host_str()
            } else {
                true
            }
        }).cloned().collect()
}

pub(crate) fn filter_host(found_urls: &mut HashSet<Url>, origin_url: &Url) {
    found_urls.retain(|url| url.host_str() == origin_url.host_str())
}

pub(crate) fn get_links_from_html(html: &Data) -> HashSet<Url> {
    if let Data::HTML(origin_url, html) = html { 
        Document::from(html.as_str())
            .find(Attr("href", ()).or(Attr("src", ())))
            .filter_map(|n| n.attr("href").or(n.attr("src")))
            .filter_map(|url| normalize_url(url, &origin_url))
            .collect::<HashSet<Url>>()
     } else {
         HashSet::new()
     }
}

fn normalize_url(url_str: &str, origin_url: &Url) -> Option<Url> {
    // println!("Before Normalization: {}", url_str);
    let url = Url::parse(url_str);

    // is absolute
    let normal = if let Ok(url) = url {
        // is valid
        if url.scheme() == "https" || url.scheme() == "http" {
            Some(url)
        } else {
            None
        }
    } else {
        let parent_url = if origin_url.as_str().ends_with('/') {
            origin_url.clone()
        } else {
            let (dir, file) = origin_url.as_str().rsplit_once('/').unwrap();
            if file.contains('.') {
                Url::parse(dir).unwrap()
            } else {
                Url::parse(&(origin_url.to_string() + "/")).unwrap()
            }
        };

        if url_str.starts_with('#') {
            None
        }else if url_str.starts_with("?") {
            if origin_url.as_str().ends_with('/') {
                Url::parse(
                    format!("{}index.html{}", origin_url, url_str).as_str()
                )
                .ok()
            } else {
                Url::parse(
                    format!("{}{}", origin_url, url_str).as_str()
                )
                .ok()
            }
        } else if url_str.starts_with("//") {
            Url::parse(
                format!("{}:{}", parent_url.scheme(), url_str).as_str()
            )
            .ok()
        } else if url_str.starts_with('/') {
            Url::parse(
                format!("{}://{}{}", parent_url.scheme(), origin_url.host_str().unwrap(), url_str).as_str()
            )
            .ok()
        } else {
            Url::parse(
                format!("{}{}", parent_url, url_str).as_str()
            )
            .ok()
        }
    };

    normal

}


pub(crate) fn link_to_path(data: &Data) -> String {
    let root = String::from("static/");
    let parent = data.get_url().host_str().unwrap();
    let (path_file, ext, url) = match data {
        Data::HTML(url, _) => {

            let path = sanitize_path(url.path());
            if path.ends_with('/')
            {
                (root + parent + &path + "index", ".html", url)
            }
            else if !path.ends_with(".html")
            {
                (root + parent + &path + "/index", ".html", url)
            }
            else
            {
                (root + parent + &path[..path.len()-5], ".html", url)
            }
        },
        Data::OTHER(url, _) => {
            // (root + parent + url.path(), url)
            let path = url.path();
            let (path, file) = path.split_at(path.rfind('/').unwrap() + 1);
            let (name, ext) = file.split_at(file.rfind('.').unwrap_or(file.len()));
            (root + parent + path + name, ext, url)
        }
    };

    // append query
    let query = if let Some(query) = url.query() {
        format!("_Q_{}", sanitize_filename::sanitize_with_options(query, SF_OPTIONS))
    } else {
        String::new()
    };

    path_file + query.as_str() + ext
}

pub(crate) fn write_to_file(data: Data) -> Result<()> {
    let data_path = link_to_path(&data);
    let (dir, _) = data_path.rsplit_once('/').unwrap();

    match data {
        Data::HTML(url, data) => {
            std::fs::create_dir_all(dir).map_err(|e| (url.clone(), e))?;
            println!("File to write: {}", data_path);
            std::fs::write(data_path, data).map_err(|e| (url, e))?;
        },
        Data::OTHER(url, data) => {
            std::fs::create_dir_all(dir).map_err(|e| (url.clone(), e))?;
            println!("File to write: {}", data_path);
            std::fs::write(data_path, data).map_err(|e| (url, e))?;
        }
    }

    Ok(())
}


fn sanitize_path(path: &str) -> String {

    path.chars()
        .map(|c| {
            match c {
                '\\' => '_',
                ':' => '_',
                '*' => '_',
                '?' => '_',
                '"' => '_',
                '<' => '_',
                '>' => '_',
                '|' => '_',
                _ => c
            }
        }).collect::<String>()
}