use crawler::ScraperBuilder;

use clap::App;
use clap::Arg;

fn main() {
    let matches = App::new("Rust Web-Crawler")
        .version("1.0")
        .about("This is a multi-threaded web-crawler \nwritten in rust that helps you scrap \ndata from any website")
        .author("vafa tarighi <vafatarighi1379@gmail.com>")
        .arg(Arg::new("URL")
        .required(true))
        .arg(Arg::new("Threads")
            .short('t')
            .long("threads")
            .takes_value(true)
            .validator(|t| t.parse::<usize>()
                .map_err(|_| "expected a positive integer value")
            )
            .help("sets number of thread used to fetch data")
        ).arg(Arg::new("Depth")
            .short('d')
            .long("depth")
            .takes_value(true)
            .validator(|t| t.parse::<usize>()
                .map_err(|_| "expected a positive integer value")
            )
            .help("incidates depth of crawling")
        ).get_matches();

        let origin_url = matches.value_of("URL").unwrap();

        let mut scraper_builder = ScraperBuilder::new(origin_url);
        if let Some(t) = matches.value_of("Threads") {
            scraper_builder = scraper_builder.threads(t.parse().unwrap());
        }
        if let Some(d) = matches.value_of("Depth") {
            scraper_builder = scraper_builder.depth(d.parse().unwrap());
        }

        let mut scraper = scraper_builder.build();

        match &mut scraper {
            Err(e) => println!("Build Error: {:#?}", e),
            Ok(scraper) => scraper.start()
        }

}
